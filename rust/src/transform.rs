use std::collections::HashSet;

use swc_common::Spanned;
use swc_ecmascript::ast;
use swc_ecmascript::visit::{Visit, VisitWith};

use crate::collect::{GlobalCollect, Id, ImportKind};
use crate::parse::TransformCodeOptions;

macro_rules! id {
    ($ident: expr) => {
        ($ident.sym.clone(), $ident.span.ctxt())
    };
}

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct TransformOutput {
    pub replaces: Vec<TransformReplaces>,
    pub removals: Vec<TransformRemovals>,
}

#[derive(Serialize, Deserialize)]
pub struct TransformReplaces {
    pub lo: u32,
    pub hi: u32,
    pub import_src: String,
    pub import_name: String,
}

#[derive(Serialize, Deserialize)]
pub struct TransformRemovals {
    pub lo: u32,
    pub hi: u32,
}

pub struct MacroTransform<'a> {
    macro_ids: Vec<Id>,
    pub replaces: Vec<TransformReplaces>,
    pub removals: Vec<TransformRemovals>,
    global_collector: &'a GlobalCollect,
}

impl<'a> MacroTransform<'a> {
    pub fn new(global_collector: &'a GlobalCollect, config: TransformCodeOptions) -> Self {
        let filter = config.filter;
        let assert_macro = config.assert_type;
        let mut removals = HashSet::new();
        let macro_ids: Vec<Id> = global_collector
            .imports
            .iter()
            .flat_map(|(id, import)| {
                let name = if import.kind == ImportKind::Default {
                    "default"
                } else {
                    &import.specifier
                };
                let assert_type = if let Some(asserts) = &import.asserts {
                    let mut assert_type: Option<String> = None;
                    for prop in asserts.props.iter() {
                        if let ast::PropOrSpread::Prop(box ast::Prop::KeyValue(key_value)) = prop {
                            match (&key_value.key, &key_value.value) {
                                (
                                    ast::PropName::Ident(ident),
                                    box ast::Expr::Lit(ast::Lit::Str(str)),
                                ) => {
                                    if &ident.sym == "type" {
                                        assert_type = Some(str.value.to_string());
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    assert_type
                } else {
                    None
                };
                if let Some(assert_type) = assert_type {
                    if assert_type == assert_macro {
                        removals.insert(import.span);
                        return Some(id.clone());
                    }
                    return None;
                }
                if filter(name.to_string(), import.source.to_string()) {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect();
        Self {
            macro_ids,
            replaces: Vec::new(),
            removals: removals
                .into_iter()
                .map(|span| TransformRemovals {
                    lo: span.lo().0 - 1,
                    hi: span.hi().0 - 1,
                })
                .collect(),
            global_collector,
        }
    }
}

impl<'a> Visit for MacroTransform<'a> {
    fn visit_expr(&mut self, node: &ast::Expr) {
        if let ast::Expr::Call(ast::CallExpr {
            callee: ast::Callee::Expr(box ast::Expr::Ident(ident)),
            ..
        }) = &node
        {
            if self.macro_ids.contains(&id!(ident)) {
                let span = node.span();
                let import = self.global_collector.imports.get(&id!(ident)).unwrap();
                self.replaces.push(TransformReplaces {
                    hi: span.hi().0 - 1,
                    lo: span.lo().0 - 1,
                    import_name: import.specifier.to_string(),
                    import_src: import.source.to_string(),
                });
            }
        }
        node.visit_children_with(self);
    }
}
