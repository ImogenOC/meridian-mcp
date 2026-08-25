//! Transport-independent DreamMaker reference analysis.
//!
//! The resolution rules are adapted from
//! `crates/dm-langserver/src/find_references.rs` at the exact revision recorded
//! by `SPACEMANDMM_REVISION`. This module intentionally retains no LSP state.

use crate::index::ReferenceKind;
use dreammaker::ast::*;
use dreammaker::objtree::{ObjectTree, ProcRef, SymbolId, TypeRef};
use dreammaker::Location;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, Default)]
pub struct ReferenceTable {
    uses: BTreeMap<SymbolId, Vec<ResolvedReference>>,
    skipped_dynamic: usize,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResolvedReference {
    pub location: Location,
    pub kind: ReferenceKind,
}

impl ReferenceTable {
    pub fn build(objtree: &ObjectTree) -> Self {
        let mut table = Self::default();
        objtree.root().recurse(&mut |ty| {
            for (name, var) in &ty.vars {
                if let Some(expression) = &var.value.expression {
                    let mut walker = ReferenceWalker::from_type(&mut table, objtree, ty);
                    let type_hint = ty.get_var_declaration(name).and_then(|declaration| {
                        walker.static_type(&declaration.var_type).basic_type()
                    });
                    walker.visit_expression(
                        var.value.location,
                        expression,
                        type_hint,
                        ReferenceKind::Read,
                    );
                }
            }
            for proc_ref in ty.iter_self_procs() {
                if let Some(code) = &proc_ref.code {
                    ReferenceWalker::from_proc(&mut table, objtree, proc_ref).run(proc_ref, code);
                }
            }
        });
        for references in table.uses.values_mut() {
            references.sort();
            references.dedup();
        }
        table
    }

    pub fn references(&self, symbol: SymbolId) -> &[ResolvedReference] {
        self.uses
            .get(&symbol)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn skipped_dynamic(&self) -> usize {
        self.skipped_dynamic
    }

    fn record(&mut self, symbol: SymbolId, location: Location, kind: ReferenceKind) {
        self.uses
            .entry(symbol)
            .or_default()
            .push(ResolvedReference { location, kind });
    }

    fn skip_dynamic(&mut self) {
        self.skipped_dynamic = self.skipped_dynamic.saturating_add(1);
    }
}

#[derive(Clone, Debug)]
enum StaticType<'o> {
    None,
    Type(TypeRef<'o>),
    List {
        list: TypeRef<'o>,
        keys: Box<StaticType<'o>>,
    },
}

impl<'o> StaticType<'o> {
    fn basic_type(&self) -> Option<TypeRef<'o>> {
        match self {
            Self::None => None,
            Self::Type(ty) => Some(*ty),
            Self::List { list, .. } => Some(*list),
        }
    }
}

struct ReferenceWalker<'o> {
    table: &'o mut ReferenceTable,
    objtree: &'o ObjectTree,
    ty: TypeRef<'o>,
    proc_ref: Option<ProcRef<'o>>,
    locals: HashMap<Ident, StaticType<'o>>,
}

impl<'o> ReferenceWalker<'o> {
    fn from_proc(
        table: &'o mut ReferenceTable,
        objtree: &'o ObjectTree,
        proc_ref: ProcRef<'o>,
    ) -> Self {
        let mut locals = HashMap::new();
        locals.insert("global".into(), StaticType::Type(objtree.root()));
        locals.insert(".".into(), StaticType::None);
        locals.insert("args".into(), StaticType::Type(objtree.expect("/list")));
        locals.insert("usr".into(), StaticType::Type(objtree.expect("/mob")));
        if !proc_ref.ty().is_root() {
            locals.insert("src".into(), StaticType::Type(proc_ref.ty()));
        }
        Self {
            table,
            objtree,
            ty: proc_ref.ty(),
            proc_ref: Some(proc_ref),
            locals,
        }
    }

    fn from_type(table: &'o mut ReferenceTable, objtree: &'o ObjectTree, ty: TypeRef<'o>) -> Self {
        let mut locals = HashMap::new();
        locals.insert("global".into(), StaticType::Type(objtree.root()));
        Self {
            table,
            objtree,
            ty,
            proc_ref: None,
            locals,
        }
    }

    fn run(&mut self, proc_ref: ProcRef<'o>, block: &'o [Spanned<Statement>]) {
        for parameter in &proc_ref.get().parameters {
            let ty = self.static_type(&parameter.var_type);
            self.record_type(parameter.location, &ty);
            if let Some(default) = &parameter.default {
                self.visit_expression(parameter.location, default, None, ReferenceKind::Read);
            }
            self.locals.insert(parameter.name.clone(), ty);
        }
        self.visit_block(block);
    }

    fn visit_block(&mut self, block: &'o [Spanned<Statement>]) {
        for statement in block {
            self.visit_statement(statement.location, &statement.elem);
        }
    }

    fn visit_statement(&mut self, location: Location, statement: &'o Statement) {
        match statement {
            Statement::Expr(expression)
            | Statement::Throw(expression)
            | Statement::Del(expression) => {
                self.visit_expression(location, expression, None, ReferenceKind::Read);
            }
            Statement::Return(expression) | Statement::Crash(expression) => {
                if let Some(expression) = expression {
                    self.visit_expression(location, expression, None, ReferenceKind::Read);
                }
            }
            Statement::While { condition, block } => {
                self.visit_expression(
                    condition.location,
                    &condition.elem,
                    None,
                    ReferenceKind::Read,
                );
                self.visit_block(block);
            }
            Statement::DoWhile { block, condition } => {
                self.visit_block(block);
                self.visit_expression(
                    condition.location,
                    &condition.elem,
                    None,
                    ReferenceKind::Read,
                );
            }
            Statement::If { arms, else_arm } => {
                for (condition, block) in arms {
                    self.visit_expression(
                        condition.location,
                        &condition.elem,
                        None,
                        ReferenceKind::Read,
                    );
                    self.visit_block(block);
                }
                if let Some(block) = else_arm {
                    self.visit_block(block);
                }
            }
            Statement::ForInfinite { block } => self.visit_block(block),
            Statement::ForLoop {
                init,
                test,
                inc,
                block,
            } => {
                if let Some(init) = init {
                    self.visit_statement(location, init);
                }
                if let Some(test) = test {
                    self.visit_expression(location, test, None, ReferenceKind::Read);
                }
                if let Some(inc) = inc {
                    self.visit_statement(location, inc);
                }
                self.visit_block(block);
            }
            Statement::ForList(for_list) => {
                if let Some(input) = &for_list.in_list {
                    self.visit_expression(location, input, None, ReferenceKind::Read);
                }
                if let Some(var_type) = &for_list.var_type {
                    self.visit_var(location, var_type, &for_list.name, None);
                }
                self.visit_block(&for_list.block);
            }
            Statement::ForRange(for_range) => {
                self.visit_expression(location, &for_range.start, None, ReferenceKind::Read);
                self.visit_expression(location, &for_range.end, None, ReferenceKind::Read);
                if let Some(step) = &for_range.step {
                    self.visit_expression(location, step, None, ReferenceKind::Read);
                }
                if let Some(var_type) = &for_range.var_type {
                    self.visit_var(location, var_type, &for_range.name, None);
                }
                self.visit_block(&for_range.block);
            }
            Statement::ForKeyValue(for_key_value) => {
                if let Some(input) = &for_key_value.in_list {
                    self.visit_expression(location, input, None, ReferenceKind::Read);
                }
                if let Some(var_type) = &for_key_value.var_type {
                    self.visit_var(location, var_type, &for_key_value.key, None);
                }
                self.locals
                    .insert(for_key_value.value.clone(), StaticType::None);
                self.visit_block(&for_key_value.block);
            }
            Statement::Var(statement) => self.visit_var_statement(location, statement),
            Statement::Vars(statements) => {
                for statement in statements {
                    self.visit_var_statement(location, statement);
                }
            }
            Statement::Setting { value, .. } => {
                self.visit_expression(location, value, None, ReferenceKind::Read);
            }
            Statement::Spawn { delay, block } => {
                if let Some(delay) = delay {
                    self.visit_expression(location, delay, None, ReferenceKind::Read);
                }
                self.visit_block(block);
            }
            Statement::Switch {
                input,
                cases,
                default,
            } => {
                self.visit_expression(location, input, None, ReferenceKind::Read);
                for (case, block) in cases.iter() {
                    for part in &case.elem {
                        match part {
                            Case::Exact(expression) => {
                                self.visit_expression(
                                    case.location,
                                    expression,
                                    None,
                                    ReferenceKind::Read,
                                );
                            }
                            Case::Range(start, end) => {
                                self.visit_expression(
                                    case.location,
                                    start,
                                    None,
                                    ReferenceKind::Read,
                                );
                                self.visit_expression(
                                    case.location,
                                    end,
                                    None,
                                    ReferenceKind::Read,
                                );
                            }
                        }
                    }
                    self.visit_block(block);
                }
                if let Some(default) = default {
                    self.visit_block(default);
                }
            }
            Statement::TryCatch {
                try_block,
                catch_params,
                catch_block,
            } => {
                self.visit_block(try_block);
                for caught in catch_params {
                    if let Some((name, path)) = caught.as_slice().split_last() {
                        let var_type: VarType = path
                            .iter()
                            .filter(|part| part.as_str() != "var")
                            .cloned()
                            .collect();
                        self.visit_var(location, &var_type, name, None);
                    }
                }
                self.visit_block(catch_block);
            }
            Statement::Label { block, .. } => self.visit_block(block),
            Statement::Continue(_) | Statement::Break(_) | Statement::Goto(_) => {}
        }
    }

    fn visit_var_statement(&mut self, location: Location, statement: &'o VarStatement) {
        self.visit_var(
            location,
            &statement.var_type,
            &statement.name,
            statement.value.as_ref(),
        );
    }

    fn visit_var(
        &mut self,
        location: Location,
        var_type: &VarType,
        name: &Ident,
        value: Option<&'o Expression>,
    ) {
        let ty = self.static_type(var_type);
        self.record_type(location, &ty);
        if let Some(value) = value {
            self.visit_expression(location, value, ty.basic_type(), ReferenceKind::Read);
        }
        self.locals.insert(name.clone(), ty);
    }

    #[allow(clippy::only_used_in_recursion)]
    fn visit_expression(
        &mut self,
        location: Location,
        expression: &'o Expression,
        type_hint: Option<TypeRef<'o>>,
        access: ReferenceKind,
    ) -> StaticType<'o> {
        match expression {
            Expression::Base { term, follow } => {
                let base_access = if follow.is_empty() {
                    access
                } else {
                    ReferenceKind::Read
                };
                let mut ty = self.visit_term(term.location, &term.elem, type_hint, base_access);
                for (index, each) in follow.iter().enumerate() {
                    let follow_access = if index + 1 == follow.len() {
                        access
                    } else {
                        ReferenceKind::Read
                    };
                    ty = self.visit_follow(each.location, ty, &each.elem, follow_access);
                }
                ty
            }
            Expression::BinaryOp { lhs, rhs, .. } => {
                self.visit_expression(location, lhs, None, ReferenceKind::Read);
                self.visit_expression(location, rhs, None, ReferenceKind::Read);
                StaticType::None
            }
            Expression::AssignOp { lhs, rhs, .. } => {
                let lhs_type = self.visit_expression(location, lhs, None, ReferenceKind::Write);
                self.visit_expression(location, rhs, lhs_type.basic_type(), ReferenceKind::Read)
            }
            Expression::TernaryOp { cond, if_, else_ } => {
                self.visit_expression(location, cond, None, ReferenceKind::Read);
                let ty = self.visit_expression(location, if_, type_hint, ReferenceKind::Read);
                self.visit_expression(location, else_, type_hint, ReferenceKind::Read);
                ty
            }
        }
    }

    fn visit_term(
        &mut self,
        location: Location,
        term: &'o Term,
        type_hint: Option<TypeRef<'o>>,
        access: ReferenceKind,
    ) -> StaticType<'o> {
        match term {
            Term::Null
            | Term::Int(_)
            | Term::Float(_)
            | Term::String(_)
            | Term::Resource(_)
            | Term::As(_) => StaticType::None,
            Term::Expr(expression) => {
                self.visit_expression(location, expression, type_hint, access)
            }
            Term::Prefab(prefab) => self
                .visit_prefab(location, prefab)
                .map_or(StaticType::None, StaticType::Type),
            Term::InterpString(_, parts) => {
                for (expression, _) in parts {
                    if let Some(expression) = expression {
                        self.visit_expression(location, expression, None, ReferenceKind::Read);
                    }
                }
                StaticType::None
            }
            Term::Ident(name) => self.visit_ident(location, name, access),
            Term::Call(name, arguments) => self.visit_unscoped_call(location, name, arguments),
            Term::SelfCall(arguments) | Term::ParentCall(arguments) => {
                self.visit_arguments(location, arguments);
                StaticType::None
            }
            Term::NewImplicit { args } => self.visit_new(location, type_hint, args),
            Term::NewPrefab { prefab, args } => {
                let ty = self.visit_prefab(location, prefab);
                self.visit_new(location, ty, args)
            }
            Term::NewMiniExpr { expr, args } => {
                let mut current = self.visit_ident(location, &expr.ident, ReferenceKind::Read);
                for field in &expr.fields {
                    current =
                        self.visit_field(location, current, &field.ident, ReferenceKind::Read);
                }
                if current.basic_type().is_none() {
                    self.table.skip_dynamic();
                }
                if let Some(args) = args {
                    self.visit_arguments(location, args);
                }
                StaticType::None
            }
            Term::List(arguments) => {
                self.visit_arguments(location, arguments);
                StaticType::List {
                    list: self.objtree.expect("/list"),
                    keys: Box::new(StaticType::None),
                }
            }
            Term::Locate { args, in_list } | Term::Input { args, in_list, .. } => {
                self.visit_arguments(location, args);
                if let Some(input) = in_list {
                    self.visit_expression(location, input, None, ReferenceKind::Read);
                }
                StaticType::None
            }
            Term::Pick(arguments) => {
                for (weight, value) in arguments.iter() {
                    if let Some(weight) = weight {
                        self.visit_expression(location, weight, None, ReferenceKind::Read);
                    }
                    self.visit_expression(location, value, None, ReferenceKind::Read);
                }
                StaticType::None
            }
            Term::DynamicCall(first, second) => {
                self.table.skip_dynamic();
                self.visit_arguments(location, first);
                self.visit_arguments(location, second);
                StaticType::None
            }
            Term::ExternalCall {
                library,
                function,
                args,
            } => {
                self.table.skip_dynamic();
                if let Some(library) = library {
                    self.visit_expression(location, library, None, ReferenceKind::Read);
                }
                self.visit_expression(location, function, None, ReferenceKind::Read);
                self.visit_arguments(location, args);
                StaticType::None
            }
            Term::GlobalCall(name, arguments) => {
                if let Some(proc_ref) = self.objtree.root().get_proc(name) {
                    self.record_proc(location, self.objtree.root(), proc_ref);
                } else {
                    self.table.skip_dynamic();
                }
                self.visit_arguments(location, arguments);
                StaticType::None
            }
            Term::GlobalIdent(name) => {
                if let Some(declaration) = self.objtree.root().get_var_declaration(name) {
                    self.table.record(declaration.id, location, access);
                    self.static_type(&declaration.var_type)
                } else {
                    StaticType::None
                }
            }
            Term::__TYPE__ => {
                self.table
                    .record(self.ty.id, location, ReferenceKind::TypePath);
                StaticType::None
            }
            Term::__PROC__ => {
                if let Some(proc_ref) = self.proc_ref {
                    if let Some(declaration) = self.ty.get_proc_declaration(proc_ref.name()) {
                        self.table
                            .record(declaration.id, location, ReferenceKind::Call);
                    }
                }
                StaticType::None
            }
            Term::__IMPLIED_TYPE__ => {
                if let Some(ty) = type_hint {
                    self.table.record(ty.id, location, ReferenceKind::TypePath);
                    StaticType::Type(ty)
                } else {
                    StaticType::None
                }
            }
        }
    }

    fn visit_ident(
        &mut self,
        location: Location,
        name: &str,
        access: ReferenceKind,
    ) -> StaticType<'o> {
        if let Some(local) = self.locals.get(name) {
            return local.clone();
        }
        if let Some(declaration) = self.ty.get_var_declaration(name) {
            self.table.record(declaration.id, location, access);
            self.static_type(&declaration.var_type)
        } else {
            StaticType::None
        }
    }

    fn visit_unscoped_call(
        &mut self,
        location: Location,
        name: &str,
        arguments: &'o [Expression],
    ) -> StaticType<'o> {
        if let Some(proc_ref) = self.ty.get_proc(name) {
            self.record_proc(location, self.ty, proc_ref);
        } else {
            self.table.skip_dynamic();
        }
        self.visit_arguments(location, arguments);
        StaticType::None
    }

    fn record_proc(&mut self, location: Location, source: TypeRef<'o>, proc_ref: ProcRef<'o>) {
        if let Some(declaration) = source.get_proc_declaration(proc_ref.name()) {
            self.table
                .record(declaration.id, location, ReferenceKind::Call);
        }
    }

    fn visit_new(
        &mut self,
        location: Location,
        ty: Option<TypeRef<'o>>,
        arguments: &'o Option<Box<[Expression]>>,
    ) -> StaticType<'o> {
        if let Some(arguments) = arguments {
            self.visit_arguments(location, arguments);
        }
        if let Some(ty) = ty {
            self.table.record(ty.id, location, ReferenceKind::TypePath);
            StaticType::Type(ty)
        } else {
            StaticType::None
        }
    }

    fn visit_prefab(&mut self, location: Location, prefab: &'o Prefab) -> Option<TypeRef<'o>> {
        let navigation = self.ty.navigate_path(prefab.path.as_slice())?;
        let ty = navigation.ty();
        self.table.record(ty.id, location, ReferenceKind::TypePath);
        for (name, expression) in &prefab.vars {
            let hint = if let Some(declaration) = ty.get_var_declaration(name) {
                self.table
                    .record(declaration.id, location, ReferenceKind::Write);
                self.static_type(&declaration.var_type).basic_type()
            } else {
                None
            };
            self.visit_expression(location, expression, hint, ReferenceKind::Read);
        }
        Some(ty)
    }

    fn visit_follow(
        &mut self,
        location: Location,
        lhs: StaticType<'o>,
        follow: &'o Follow,
        access: ReferenceKind,
    ) -> StaticType<'o> {
        match follow {
            Follow::Unary(_) => StaticType::None,
            Follow::Index(_, expression) => {
                self.visit_expression(location, expression, None, ReferenceKind::Read);
                match lhs {
                    StaticType::List { keys, .. } => *keys,
                    _ => StaticType::None,
                }
            }
            Follow::Field(_, name) | Follow::StaticField(name) => {
                self.visit_field(location, lhs, name, access)
            }
            Follow::Call(_, name, arguments) => {
                if let Some(ty) = lhs.basic_type() {
                    if let Some(proc_ref) = ty.get_proc(name) {
                        self.record_proc(location, ty, proc_ref);
                    } else {
                        self.table.skip_dynamic();
                    }
                } else {
                    self.table.skip_dynamic();
                }
                self.visit_arguments(location, arguments);
                StaticType::None
            }
            Follow::ProcReference(name) => {
                if let Some(ty) = lhs.basic_type() {
                    if let Some(declaration) = ty.get_proc_declaration(name) {
                        self.table
                            .record(declaration.id, location, ReferenceKind::Call);
                    } else {
                        self.table.skip_dynamic();
                    }
                } else {
                    self.table.skip_dynamic();
                }
                StaticType::None
            }
        }
    }

    fn visit_field(
        &mut self,
        location: Location,
        lhs: StaticType<'o>,
        name: &str,
        access: ReferenceKind,
    ) -> StaticType<'o> {
        if let Some(ty) = lhs.basic_type() {
            if let Some(declaration) = ty.get_var_declaration(name) {
                self.table.record(declaration.id, location, access);
                return self.static_type(&declaration.var_type);
            }
        }
        self.table.skip_dynamic();
        StaticType::None
    }

    fn visit_arguments(&mut self, location: Location, arguments: &'o [Expression]) {
        for argument in arguments {
            let value = match argument {
                Expression::AssignOp {
                    op: AssignOp::Assign,
                    lhs,
                    rhs,
                } if lhs.as_term().and_then(Term::as_kwarg_key).is_some() => rhs,
                _ => argument,
            };
            self.visit_expression(location, value, None, ReferenceKind::Read);
        }
    }

    fn record_type(&mut self, location: Location, ty: &StaticType<'o>) {
        match ty {
            StaticType::None => {}
            StaticType::Type(ty) => self.table.record(ty.id, location, ReferenceKind::TypePath),
            StaticType::List { list, keys } => {
                self.table
                    .record(list.id, location, ReferenceKind::TypePath);
                self.record_type(location, keys);
            }
        }
    }

    fn static_type(&self, var_type: &VarType) -> StaticType<'o> {
        let mut path = var_type.type_path.as_slice();
        while let Some(first) = path.first() {
            if ![
                "static",
                "global",
                "const",
                "tmp",
                "final",
                "SpacemanDMM_final",
                "SpacemanDMM_private",
                "SpacemanDMM_protected",
            ]
            .contains(&first.as_str())
            {
                break;
            }
            path = &path[1..];
        }
        if path.is_empty() {
            StaticType::None
        } else if path[0] == "list" {
            let nested: VarType = path[1..].iter().cloned().collect();
            StaticType::List {
                list: self.objtree.expect("/list"),
                keys: Box::new(self.static_type(&nested)),
            }
        } else {
            self.objtree
                .type_by_path(path)
                .map_or(StaticType::None, StaticType::Type)
        }
    }
}
