use std::collections::HashMap;

use swc_atoms::{js_word, JsWord};
use swc_common::{Span, SyntaxContext};
use swc_ecmascript::ast;
use swc_ecmascript::visit::{noop_visit_type, Visit, VisitWith};

macro_rules! id {
    ($ident: expr) => {
        ($ident.sym.clone(), $ident.span.ctxt())
    };
}

pub type Id = (JsWord, SyntaxContext);

#[derive(Eq, PartialEq, Clone, Copy)]
pub enum ImportKind {
    Named,
    All,
    Default,
}

#[derive(Clone)]
pub struct Import {
    pub source: JsWord,
    pub specifier: JsWord,
    pub kind: ImportKind,
    pub synthetic: bool,
    pub asserts: Option<Box<ast::ObjectLit>>,
}

pub struct GlobalCollect {
    pub synthetic: Vec<(Id, Import)>,
    pub imports: HashMap<Id, Import>,
    pub exports: HashMap<Id, Option<JsWord>>,
    pub root: HashMap<Id, Span>,

    rev_imports: HashMap<(JsWord, JsWord), Id>,
    in_export_decl: bool,
}

pub fn global_collect(module: &ast::Module) -> GlobalCollect {
    let mut collect = GlobalCollect {
        synthetic: vec![],
        imports: HashMap::with_capacity(16),
        exports: HashMap::with_capacity(16),

        root: HashMap::with_capacity(16),
        rev_imports: HashMap::with_capacity(16),

        in_export_decl: false,
    };
    module.visit_with(&mut collect);
    collect
}

impl GlobalCollect {
    pub fn add_import(&mut self, local: Id, import: Import) {
        if import.synthetic {
            self.synthetic.push((local.clone(), import.clone()));
        }
        self.rev_imports.insert(
            (import.specifier.clone(), import.source.clone()),
            local.clone(),
        );
        self.imports.insert(local, import);
    }

    pub fn add_export(&mut self, local: Id, exported: Option<JsWord>) -> bool {
        if let std::collections::hash_map::Entry::Vacant(e) = self.exports.entry(local) {
            e.insert(exported);
            true
        } else {
            false
        }
    }
}

impl Visit for GlobalCollect {
    noop_visit_type!();

    fn visit_module_item(&mut self, node: &ast::ModuleItem) {
        if let ast::ModuleItem::Stmt(ast::Stmt::Decl(decl)) = node {
            match decl {
                ast::Decl::Fn(function) => {
                    self.root.insert(id!(function.ident), function.ident.span);
                }
                ast::Decl::Class(class) => {
                    self.root.insert(id!(class.ident), class.ident.span);
                }
                ast::Decl::Var(var) => {
                    for decl in &var.decls {
                        let mut identifiers: Vec<(Id, Span)> = vec![];
                        collect_from_pat(&decl.name, &mut identifiers);
                        self.root.extend(identifiers.into_iter());
                    }
                }
                ast::Decl::TsEnum(enu) => {
                    self.root.insert(id!(enu.id), enu.id.span);
                }
                _ => {}
            }
        } else {
            node.visit_children_with(self);
        }
    }

    fn visit_import_decl(&mut self, node: &ast::ImportDecl) {
        for specifier in &node.specifiers {
            match specifier {
                ast::ImportSpecifier::Named(named) => {
                    let imported = match &named.imported {
                        Some(ast::ModuleExportName::Ident(ident)) => ident.sym.clone(),
                        _ => named.local.sym.clone(),
                    };
                    self.add_import(
                        id!(named.local),
                        Import {
                            source: node.src.value.clone(),
                            specifier: imported,
                            kind: ImportKind::Named,
                            synthetic: false,
                            asserts: node.asserts.clone(),
                        },
                    );
                }
                ast::ImportSpecifier::Default(default) => {
                    self.add_import(
                        id!(default.local),
                        Import {
                            source: node.src.value.clone(),
                            specifier: js_word!("default"),
                            kind: ImportKind::Default,
                            synthetic: false,
                            asserts: node.asserts.clone(),
                        },
                    );
                }
                ast::ImportSpecifier::Namespace(namespace) => {
                    self.add_import(
                        id!(namespace.local),
                        Import {
                            source: node.src.value.clone(),
                            specifier: "*".into(),
                            kind: ImportKind::All,
                            synthetic: false,
                            asserts: node.asserts.clone(),
                        },
                    );
                }
            }
        }
    }

    fn visit_named_export(&mut self, node: &ast::NamedExport) {
        if node.src.is_some() {
            return;
        }

        for specifier in &node.specifiers {
            match specifier {
                ast::ExportSpecifier::Named(named) => {
                    let local = match &named.orig {
                        ast::ModuleExportName::Ident(ident) => Some(id!(ident)),
                        _ => None,
                    };
                    let exported = match &named.exported {
                        Some(ast::ModuleExportName::Ident(exported)) => Some(exported.sym.clone()),
                        _ => None,
                    };
                    if let Some(local) = local {
                        self.add_export(local, exported);
                    }
                }
                ast::ExportSpecifier::Default(default) => {
                    self.exports
                        .entry(id!(default.exported))
                        .or_insert(Some(js_word!("default")));
                }
                ast::ExportSpecifier::Namespace(namespace) => {
                    if let ast::ModuleExportName::Ident(ident) = &namespace.name {
                        self.exports
                            .entry(id!(ident))
                            .or_insert_with(|| Some("*".into()));
                    }
                }
            }
        }
    }

    fn visit_export_decl(&mut self, node: &ast::ExportDecl) {
        match &node.decl {
            ast::Decl::TsEnum(enu) => {
                self.add_export(id!(enu.id), None);
            }
            ast::Decl::Class(class) => {
                self.add_export(id!(class.ident), None);
            }
            ast::Decl::Fn(func) => {
                self.add_export(id!(func.ident), None);
            }
            ast::Decl::Var(var) => {
                for decl in &var.decls {
                    self.in_export_decl = true;
                    decl.name.visit_with(self);
                    self.in_export_decl = false;

                    decl.init.visit_with(self);
                }
            }
            _ => {}
        }
    }

    fn visit_export_default_decl(&mut self, node: &ast::ExportDefaultDecl) {
        match &node.decl {
            ast::DefaultDecl::Class(class) => {
                if let Some(ident) = &class.ident {
                    self.add_export(id!(ident), Some(js_word!("default")));
                }
            }
            ast::DefaultDecl::Fn(func) => {
                if let Some(ident) = &func.ident {
                    self.add_export(id!(ident), Some(js_word!("default")));
                }
            }
            _ => {
                unreachable!("unsupported export default declaration");
            }
        };
    }

    fn visit_binding_ident(&mut self, node: &ast::BindingIdent) {
        if self.in_export_decl {
            self.add_export(id!(node.id), None);
        }
    }

    fn visit_assign_pat_prop(&mut self, node: &ast::AssignPatProp) {
        if self.in_export_decl {
            self.add_export(id!(node.key), None);
        }
    }
}

pub fn collect_from_pat(pat: &ast::Pat, identifiers: &mut Vec<(Id, Span)>) -> bool {
    match pat {
        ast::Pat::Ident(ident) => {
            identifiers.push((id!(ident.id), ident.id.span));
            true
        }
        ast::Pat::Array(array) => {
            for el in array.elems.iter().flatten() {
                collect_from_pat(el, identifiers);
            }
            false
        }
        ast::Pat::Rest(rest) => {
            if let ast::Pat::Ident(ident) = rest.arg.as_ref() {
                identifiers.push((id!(ident.id), ident.id.span));
            }
            false
        }
        ast::Pat::Assign(expr) => {
            if let ast::Pat::Ident(ident) = expr.left.as_ref() {
                identifiers.push((id!(ident.id), ident.id.span));
            }
            false
        }
        ast::Pat::Object(obj) => {
            for prop in &obj.props {
                match prop {
                    ast::ObjectPatProp::Assign(ref v) => {
                        identifiers.push((id!(v.key), v.key.span));
                    }
                    ast::ObjectPatProp::KeyValue(ref v) => {
                        collect_from_pat(&v.value, identifiers);
                    }
                    ast::ObjectPatProp::Rest(ref v) => {
                        if let ast::Pat::Ident(ident) = v.arg.as_ref() {
                            identifiers.push((id!(ident.id), ident.id.span));
                        }
                    }
                }
            }
            false
        }
        _ => false,
    }
}
