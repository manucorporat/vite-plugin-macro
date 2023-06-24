use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use swc_common::input::StringInput;
use swc_common::{sync::Lrc, FileName, Globals, Mark, SourceMap};
use swc_ecmascript::ast;
use swc_ecmascript::parser::lexer::Lexer;
use swc_ecmascript::parser::{EsConfig, PResult, Parser, Syntax, TsConfig};
use swc_ecmascript::transforms::resolver;
use swc_ecmascript::visit::{VisitMutWith, VisitWith};

use crate::collect::global_collect;
use crate::transform::{MacroTransform, TransformOutput};

pub struct TransformCodeOptions {
    pub absolute_path: String,
    pub code: String,
    pub assert_type: String,
    pub filter: Box<dyn Fn(String, String) -> bool>,
}

pub fn transform_code(config: TransformCodeOptions) -> PResult<TransformOutput> {
    let source_map = Lrc::new(SourceMap::default());

    let (mut main_module, is_type_script, _) = parse(&config, Lrc::clone(&source_map))?;

    swc_common::GLOBALS.set(&Globals::new(), || {
        let unresolved_mark = Mark::new();
        let top_level_mark = Mark::new();

        // Resolve with mark
        main_module.visit_mut_with(&mut resolver(
            unresolved_mark,
            top_level_mark,
            is_type_script,
        ));
        // Collect import/export metadata
        let collect = global_collect(&main_module);
        let mut macro_transform = MacroTransform::new(&collect, config);
        main_module.visit_with(&mut macro_transform);
        Ok(TransformOutput {
            replaces: macro_transform.replaces,
            removals: macro_transform.removals,
        })
    })
}

fn parse_filename(src: &str) -> (bool, bool) {
    let path = Path::new(src);
    let extension = path.extension().and_then(OsStr::to_str).unwrap();

    match extension {
        "ts" => (true, false),
        "mts" => (true, false),
        "mtsx" => (true, true),
        "js" => (false, false),
        "mjs" => (false, false),
        "cjs" => (false, false),
        "jsx" => (false, true),
        "mjsx" => (false, true),
        "cjsx" => (false, true),
        _ => (true, true),
    }
}

fn parse(
    config: &TransformCodeOptions,
    source_map: Lrc<SourceMap>,
) -> PResult<(ast::Module, bool, bool)> {
    let (is_type_script, is_jsx) = parse_filename(&config.absolute_path);
    let path_abs = PathBuf::from(&config.absolute_path);
    let source_file = source_map.new_source_file(FileName::Real(path_abs), config.code.clone());

    let syntax = if is_type_script {
        Syntax::Typescript(TsConfig {
            tsx: is_jsx,
            ..Default::default()
        })
    } else {
        Syntax::Es(EsConfig {
            jsx: is_jsx,
            export_default_from: true,
            ..Default::default()
        })
    };

    let lexer = Lexer::new(
        syntax,
        Default::default(),
        StringInput::from(&*source_file),
        None,
    );

    let mut parser = Parser::new_from(lexer);
    let module = parser.parse_module()?;
    Ok((module, is_type_script, is_jsx))
}
