//! Transformer / Transpiler
//!
//! References:
//! * <https://www.typescriptlang.org/tsconfig#target>
//! * <https://babel.dev/docs/presets>
//! * <https://github.com/microsoft/TypeScript/blob/main/src/compiler/transformer.ts>

use std::path::Path;

use oxc_allocator::{Allocator, Vec as ArenaVec};
use oxc_ast::{ast::*, AstBuilder};
use oxc_diagnostics::OxcDiagnostic;
use oxc_semantic::{ScopeTree, SymbolTable};
use oxc_span::SPAN;
use oxc_traverse::{traverse_mut, Traverse, TraverseCtx};

// Core
mod common;
mod compiler_assumptions;
mod context;
mod options;

// Presets: <https://babel.dev/docs/presets>
mod es2015;
mod es2016;
mod es2017;
mod es2018;
mod es2019;
mod es2020;
mod es2021;
mod es2022;
mod jsx;
mod regexp;
mod typescript;

mod plugins;

use common::Common;
use context::TransformCtx;
use es2015::ES2015;
use es2016::ES2016;
use es2017::ES2017;
use es2018::ES2018;
use es2019::ES2019;
use es2020::ES2020;
use es2021::ES2021;
use es2022::ES2022;
use jsx::Jsx;
use regexp::RegExp;
use typescript::TypeScript;

pub use crate::{
    common::helper_loader::HelperLoaderMode,
    compiler_assumptions::CompilerAssumptions,
    es2015::{ArrowFunctionsOptions, ES2015Options},
    jsx::{JsxOptions, JsxRuntime, ReactRefreshOptions},
    options::{
        babel::{BabelEnvOptions, BabelOptions},
        ESTarget, Engine, EngineTargets, EnvOptions, Module, TransformOptions,
    },
    plugins::*,
    typescript::{RewriteExtensionsMode, TypeScriptOptions},
};

pub struct TransformerReturn {
    pub errors: std::vec::Vec<OxcDiagnostic>,
    pub symbols: SymbolTable,
    pub scopes: ScopeTree,
}

pub struct Transformer<'a> {
    ctx: TransformCtx<'a>,
    // options: TransformOptions,
    allocator: &'a Allocator,

    typescript: TypeScriptOptions,
    jsx: JsxOptions,
    env: EnvOptions,
}

impl<'a> Transformer<'a> {
    pub fn new(allocator: &'a Allocator, source_path: &Path, options: &TransformOptions) -> Self {
        let ctx = TransformCtx::new(source_path, options);
        Self {
            ctx,
            allocator,
            typescript: options.typescript.clone(),
            jsx: options.jsx.clone(),
            env: options.env,
        }
    }

    pub fn build_with_symbols_and_scopes(
        mut self,
        symbols: SymbolTable,
        scopes: ScopeTree,
        program: &mut Program<'a>,
    ) -> TransformerReturn {
        let allocator = self.allocator;
        let ast_builder = AstBuilder::new(allocator);

        self.ctx.source_type = program.source_type;
        self.ctx.source_text = program.source_text;
        jsx::update_options_with_comments(
            &program.comments,
            &mut self.typescript,
            &mut self.jsx,
            &self.ctx,
        );

        let mut transformer = TransformerImpl {
            common: Common::new(&self.env, &self.ctx),
            x0_typescript: program
                .source_type
                .is_typescript()
                .then(|| TypeScript::new(&self.typescript, &self.ctx)),
            x1_jsx: Jsx::new(self.jsx, ast_builder, &self.ctx),
            x2_es2022: ES2022::new(self.env.es2022, &self.ctx),
            x2_es2021: ES2021::new(self.env.es2021, &self.ctx),
            x2_es2020: ES2020::new(self.env.es2020, &self.ctx),
            x2_es2019: ES2019::new(self.env.es2019),
            x2_es2018: ES2018::new(self.env.es2018, &self.ctx),
            x2_es2016: ES2016::new(self.env.es2016, &self.ctx),
            x2_es2017: ES2017::new(self.env.es2017, &self.ctx),
            x3_es2015: ES2015::new(self.env.es2015, &self.ctx),
            x4_regexp: RegExp::new(self.env.regexp, &self.ctx),
        };

        let (symbols, scopes) = traverse_mut(&mut transformer, allocator, program, symbols, scopes);
        TransformerReturn { errors: self.ctx.take_errors(), symbols, scopes }
    }
}

struct TransformerImpl<'a, 'ctx> {
    // NOTE: all callbacks must run in order.
    x0_typescript: Option<TypeScript<'a, 'ctx>>,
    x1_jsx: Jsx<'a, 'ctx>,
    x2_es2022: ES2022<'a, 'ctx>,
    x2_es2021: ES2021<'a, 'ctx>,
    x2_es2020: ES2020<'a, 'ctx>,
    x2_es2019: ES2019,
    x2_es2018: ES2018<'a, 'ctx>,
    x2_es2017: ES2017<'a, 'ctx>,
    x2_es2016: ES2016<'a, 'ctx>,
    #[expect(unused)]
    x3_es2015: ES2015<'a, 'ctx>,
    x4_regexp: RegExp<'a, 'ctx>,
    common: Common<'a, 'ctx>,
}

impl<'a, 'ctx> Traverse<'a> for TransformerImpl<'a, 'ctx> {
    fn enter_program(&mut self, program: &mut Program<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_program(program, ctx);
        }
        self.x1_jsx.enter_program(program, ctx);
    }

    fn exit_program(&mut self, program: &mut Program<'a>, ctx: &mut TraverseCtx<'a>) {
        self.x1_jsx.exit_program(program, ctx);
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.exit_program(program, ctx);
        }
        self.common.exit_program(program, ctx);
    }

    // ALPHASORT

    fn enter_arrow_function_expression(
        &mut self,
        arrow: &mut ArrowFunctionExpression<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_arrow_function_expression(arrow, ctx);
        }
    }

    fn enter_variable_declarator(
        &mut self,
        decl: &mut VariableDeclarator<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_variable_declarator(decl, ctx);
        }
    }

    fn enter_big_int_literal(&mut self, node: &mut BigIntLiteral<'a>, ctx: &mut TraverseCtx<'a>) {
        self.x2_es2020.enter_big_int_literal(node, ctx);
    }

    fn enter_binding_pattern(&mut self, pat: &mut BindingPattern<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_binding_pattern(pat, ctx);
        }
    }

    fn enter_call_expression(&mut self, expr: &mut CallExpression<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_call_expression(expr, ctx);
        }
        self.x1_jsx.enter_call_expression(expr, ctx);
    }

    fn enter_class(&mut self, class: &mut Class<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_class(class, ctx);
        }
    }

    fn enter_class_body(&mut self, body: &mut ClassBody<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_class_body(body, ctx);
        }
        self.x2_es2022.enter_class_body(body, ctx);
    }

    fn enter_static_block(&mut self, block: &mut StaticBlock<'a>, ctx: &mut TraverseCtx<'a>) {
        self.common.enter_static_block(block, ctx);
    }

    fn exit_static_block(&mut self, block: &mut StaticBlock<'a>, ctx: &mut TraverseCtx<'a>) {
        self.common.exit_static_block(block, ctx);
    }

    fn enter_ts_module_declaration(
        &mut self,
        decl: &mut TSModuleDeclaration<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_ts_module_declaration(decl, ctx);
        }
    }

    #[inline]
    fn enter_expression(&mut self, expr: &mut Expression<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_expression(expr, ctx);
        }
        self.x2_es2021.enter_expression(expr, ctx);
        self.x2_es2020.enter_expression(expr, ctx);
        self.x2_es2018.enter_expression(expr, ctx);
        self.x2_es2016.enter_expression(expr, ctx);
        self.x4_regexp.enter_expression(expr, ctx);
        self.common.enter_expression(expr, ctx);
    }

    fn exit_expression(&mut self, expr: &mut Expression<'a>, ctx: &mut TraverseCtx<'a>) {
        self.x1_jsx.exit_expression(expr, ctx);
        self.x2_es2018.exit_expression(expr, ctx);
        self.x2_es2017.exit_expression(expr, ctx);
        self.common.exit_expression(expr, ctx);
    }

    fn enter_simple_assignment_target(
        &mut self,
        node: &mut SimpleAssignmentTarget<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_simple_assignment_target(node, ctx);
        }
    }

    fn enter_assignment_target(
        &mut self,
        node: &mut AssignmentTarget<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_assignment_target(node, ctx);
        }
    }

    fn enter_formal_parameter(
        &mut self,
        param: &mut FormalParameter<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_formal_parameter(param, ctx);
        }
    }

    fn enter_function(&mut self, func: &mut Function<'a>, ctx: &mut TraverseCtx<'a>) {
        self.common.enter_function(func, ctx);
    }

    fn exit_function(&mut self, func: &mut Function<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.exit_function(func, ctx);
        }
        self.x1_jsx.exit_function(func, ctx);
        self.x2_es2018.exit_function(func, ctx);
        self.x2_es2017.exit_function(func, ctx);
        self.common.exit_function(func, ctx);
    }

    fn enter_jsx_element(&mut self, node: &mut JSXElement<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_jsx_element(node, ctx);
        }
    }

    fn enter_jsx_element_name(&mut self, node: &mut JSXElementName<'a>, ctx: &mut TraverseCtx<'a>) {
        self.common.enter_jsx_element_name(node, ctx);
    }

    fn enter_jsx_member_expression_object(
        &mut self,
        node: &mut JSXMemberExpressionObject<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        self.common.enter_jsx_member_expression_object(node, ctx);
    }

    fn enter_jsx_fragment(&mut self, node: &mut JSXFragment<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_jsx_fragment(node, ctx);
        }
    }

    fn enter_jsx_opening_element(
        &mut self,
        elem: &mut JSXOpeningElement<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_jsx_opening_element(elem, ctx);
        }
        self.x1_jsx.enter_jsx_opening_element(elem, ctx);
    }

    fn enter_method_definition(
        &mut self,
        def: &mut MethodDefinition<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_method_definition(def, ctx);
        }
    }

    fn exit_method_definition(
        &mut self,
        def: &mut MethodDefinition<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.exit_method_definition(def, ctx);
        }
    }

    fn enter_new_expression(&mut self, expr: &mut NewExpression<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_new_expression(expr, ctx);
        }
    }

    fn enter_property_definition(
        &mut self,
        def: &mut PropertyDefinition<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_property_definition(def, ctx);
        }
    }

    fn enter_accessor_property(
        &mut self,
        node: &mut AccessorProperty<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_accessor_property(node, ctx);
        }
    }

    fn enter_statements(
        &mut self,
        stmts: &mut ArenaVec<'a, Statement<'a>>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        self.common.enter_statements(stmts, ctx);
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_statements(stmts, ctx);
        }
    }

    fn exit_arrow_function_expression(
        &mut self,
        arrow: &mut ArrowFunctionExpression<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        // Some plugins may add new statements to the ArrowFunctionExpression's body,
        // which can cause issues with the `() => x;` case, as it only allows a single statement.
        // To address this, we wrap the last statement in a return statement and set the expression to false.
        // This transforms the arrow function into the form `() => { return x; };`.
        if arrow.expression && arrow.body.statements.len() > 1 {
            let Statement::ExpressionStatement(statement) = arrow.body.statements.pop().unwrap()
            else {
                unreachable!(
                    "The last statement in an ArrowFunctionExpression should always be an ExpressionStatement."
                )
            };
            arrow
                .body
                .statements
                .push(ctx.ast.statement_return(SPAN, Some(statement.unbox().expression)));
            arrow.expression = false;
        }
    }

    fn exit_statements(
        &mut self,
        stmts: &mut ArenaVec<'a, Statement<'a>>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.exit_statements(stmts, ctx);
        }
        self.common.exit_statements(stmts, ctx);
    }

    fn exit_statement(&mut self, stmt: &mut Statement<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.exit_statement(stmt, ctx);
        }
        self.x2_es2018.exit_statement(stmt, ctx);
        self.x2_es2017.exit_statement(stmt, ctx);
    }

    fn enter_tagged_template_expression(
        &mut self,
        expr: &mut TaggedTemplateExpression<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_tagged_template_expression(expr, ctx);
        }
    }

    fn enter_statement(&mut self, stmt: &mut Statement<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_statement(stmt, ctx);
        }
        self.x2_es2018.enter_statement(stmt, ctx);
    }

    fn enter_declaration(&mut self, decl: &mut Declaration<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_declaration(decl, ctx);
        }
    }

    fn enter_if_statement(&mut self, stmt: &mut IfStatement<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_if_statement(stmt, ctx);
        }
    }

    fn enter_while_statement(&mut self, stmt: &mut WhileStatement<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_while_statement(stmt, ctx);
        }
    }

    fn enter_do_while_statement(
        &mut self,
        stmt: &mut DoWhileStatement<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_do_while_statement(stmt, ctx);
        }
    }

    fn enter_for_statement(&mut self, stmt: &mut ForStatement<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_for_statement(stmt, ctx);
        }
    }

    fn enter_for_of_statement(&mut self, stmt: &mut ForOfStatement<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_for_of_statement(stmt, ctx);
        }
        self.x2_es2018.enter_for_of_statement(stmt, ctx);
    }

    fn enter_for_in_statement(&mut self, stmt: &mut ForInStatement<'a>, ctx: &mut TraverseCtx<'a>) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_for_in_statement(stmt, ctx);
        }
    }

    fn enter_catch_clause(&mut self, clause: &mut CatchClause<'a>, ctx: &mut TraverseCtx<'a>) {
        self.x2_es2019.enter_catch_clause(clause, ctx);
    }

    fn enter_import_declaration(
        &mut self,
        node: &mut ImportDeclaration<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_import_declaration(node, ctx);
        }
    }

    fn enter_export_all_declaration(
        &mut self,
        node: &mut ExportAllDeclaration<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_export_all_declaration(node, ctx);
        }
    }

    fn enter_export_named_declaration(
        &mut self,
        node: &mut ExportNamedDeclaration<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_export_named_declaration(node, ctx);
        }
    }

    fn enter_ts_export_assignment(
        &mut self,
        export_assignment: &mut TSExportAssignment<'a>,
        ctx: &mut TraverseCtx<'a>,
    ) {
        if let Some(typescript) = self.x0_typescript.as_mut() {
            typescript.enter_ts_export_assignment(export_assignment, ctx);
        }
    }
}
