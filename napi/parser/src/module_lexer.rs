use napi::{bindgen_prelude::AsyncTask, Task};
use napi_derive::napi;

use oxc::allocator::Allocator;
use oxc_module_lexer::ImportType;

use crate::{parse, ParserOptions};

#[napi(object)]
pub struct ModuleLexerImportSpecifier {
    /// Module name
    ///
    /// To handle escape sequences in specifier strings, the .n field of imported specifiers will be provided where possible.
    ///
    /// For dynamic import expressions, this field will be empty if not a valid JS string.
    pub n: Option<String>,

    /// Start of module specifier
    pub s: u32,

    /// End of module specifier
    pub e: u32,

    /// Start of import statement
    pub ss: u32,

    /// End of import statement
    pub se: u32,

    /// Import Type
    /// * If this import keyword is a dynamic import, this is the start value.
    /// * If this import keyword is a static import, this is -1.
    /// * If this import keyword is an import.meta expression, this is -2.
    /// * If this import is an `export *`, this is -3.
    pub d: i64,

    /// If this import has an import assertion, this is the start value
    /// Otherwise this is `-1`.
    pub a: i64,
}

#[napi(object)]
pub struct ModuleLexerExportSpecifier {
    /// Exported name
    pub n: String,

    /// Local name, or undefined.
    pub ln: Option<String>,

    /// Start of exported name
    pub s: u32,

    /// End of exported name
    pub e: u32,

    /// Start of local name
    pub ls: Option<u32>,

    /// End of local name
    pub le: Option<u32>,
}

impl<'a> From<oxc_module_lexer::ImportSpecifier<'a>> for ModuleLexerImportSpecifier {
    #[allow(clippy::cast_lossless)]
    fn from(i: oxc_module_lexer::ImportSpecifier) -> Self {
        Self {
            n: i.n.map(|n| n.to_string()),
            s: i.s,
            e: i.e,
            ss: i.ss,
            se: i.se,
            d: match i.d {
                ImportType::DynamicImport(start) => start as i64,
                ImportType::StaticImport => -1,
                ImportType::ImportMeta => -2,
                ImportType::ExportStar => -3,
            },
            a: i.a.map_or(-1, |a| a as i64),
        }
    }
}

impl<'a> From<oxc_module_lexer::ExportSpecifier<'a>> for ModuleLexerExportSpecifier {
    fn from(e: oxc_module_lexer::ExportSpecifier) -> Self {
        Self {
            n: e.n.to_string(),
            ln: e.ln.map(|ln| ln.to_string()),
            s: e.s,
            e: e.e,
            ls: e.ls,
            le: e.le,
        }
    }
}

#[napi(object)]
pub struct ModuleLexer {
    pub imports: Vec<ModuleLexerImportSpecifier>,

    pub exports: Vec<ModuleLexerExportSpecifier>,

    /// ESM syntax detection
    ///
    /// The use of ESM syntax: import / export statements and `import.meta`
    pub has_module_syntax: bool,

    /// Facade modules that only use import / export syntax
    pub facade: bool,
}

#[allow(clippy::needless_pass_by_value)]
fn module_lexer(source_text: &str, options: &ParserOptions) -> ModuleLexer {
    let allocator = Allocator::default();
    let ret = parse(&allocator, source_text, options);
    let module_lexer = oxc_module_lexer::ModuleLexer::new().build(&ret.program);
    let imports = module_lexer.imports.into_iter().map(ModuleLexerImportSpecifier::from).collect();
    let exports = module_lexer.exports.into_iter().map(ModuleLexerExportSpecifier::from).collect();
    ModuleLexer {
        imports,
        exports,
        has_module_syntax: module_lexer.has_module_syntax,
        facade: module_lexer.facade,
    }
}

/// Outputs the list of exports and locations of import specifiers,
/// including dynamic import and import meta handling.
///
/// # Panics
///
/// * File extension is invalid
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn module_lexer_sync(source_text: String, options: Option<ParserOptions>) -> ModuleLexer {
    let options = options.unwrap_or_default();
    module_lexer(&source_text, &options)
}

pub struct ResolveTask {
    source_text: String,
    options: ParserOptions,
}

#[napi]
impl Task for ResolveTask {
    type JsValue = ModuleLexer;
    type Output = ModuleLexer;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        Ok(module_lexer(&self.source_text, &self.options))
    }

    fn resolve(&mut self, _: napi::Env, result: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(result)
    }
}

/// # Panics
///
/// * Tokio crashes
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn module_lexer_async(
    source_text: String,
    options: Option<ParserOptions>,
) -> AsyncTask<ResolveTask> {
    let options = options.unwrap_or_default();
    AsyncTask::new(ResolveTask { source_text, options })
}
