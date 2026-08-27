//! Syntax-aware proof that every downstream winner delegates to `fava-state`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use quote::ToTokens;
use syn::visit::{self, Visit};
use syn::{
    Block, Expr, ExprClosure, ExprIf, ExprMatch, ExprMethodCall, FnArg, ImplItemFn, ItemFn,
    ItemMod, ItemUse, Local, Pat, ReturnType, Type, UseTree,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FunctionId {
    path: String,
    module: String,
    owner: Option<String>,
    name: String,
    signature: String,
}

#[derive(Clone)]
struct Function {
    id: FunctionId,
    block: Block,
    arity: usize,
    parameter_order: Vec<ValueKind>,
    parameters: BTreeMap<String, ValueKind>,
}

struct Functions {
    path: String,
    modules: Vec<String>,
    owner: Option<String>,
    values: Vec<Function>,
}

impl Functions {
    fn new(path: String) -> Self {
        Self {
            modules: source_module(&path),
            path,
            owner: None,
            values: Vec::new(),
        }
    }

    fn module(&self) -> String {
        self.modules.join("::")
    }
}

impl<'ast> Visit<'ast> for Functions {
    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        if let Some((_, items)) = &node.content {
            self.modules.push(node.ident.to_string());
            for item in items {
                self.visit_item(item);
            }
            self.modules.pop();
        }
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        let previous = self.owner.replace(normalize(&node.self_ty));
        visit::visit_item_impl(self, node);
        self.owner = previous;
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        self.values.push(Function {
            id: FunctionId {
                path: self.path.clone(),
                module: self.module(),
                owner: self.owner.clone(),
                name: node.sig.ident.to_string(),
                signature: normalize(&node.sig),
            },
            block: node.block.clone(),
            arity: node.sig.inputs.len(),
            parameter_order: ordered_parameter_kinds(&node.sig.inputs),
            parameters: parameter_kinds(&node.sig.inputs, &node.sig.output),
        });
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        self.values.push(Function {
            id: FunctionId {
                path: self.path.clone(),
                module: self.module(),
                owner: None,
                name: node.sig.ident.to_string(),
                signature: normalize(&node.sig),
            },
            block: *node.block.clone(),
            arity: node.sig.inputs.len(),
            parameter_order: ordered_parameter_kinds(&node.sig.inputs),
            parameters: parameter_kinds(&node.sig.inputs, &node.sig.output),
        });
        visit::visit_item_fn(self, node);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ValueKind {
    Timestamp,
    EventId,
    EventOrderKey,
    Other,
}

fn type_kind(ty: &Type) -> ValueKind {
    match ty {
        Type::Paren(paren) => type_kind(&paren.elem),
        Type::Reference(reference) => type_kind(&reference.elem),
        Type::Tuple(tuple) if tuple.elems.len() == 2 => {
            let mut elements = tuple.elems.iter();
            if elements
                .next()
                .is_some_and(|item| type_kind(item) == ValueKind::Timestamp)
                && elements
                    .next()
                    .is_some_and(|item| type_kind(item) == ValueKind::EventId)
            {
                ValueKind::EventOrderKey
            } else {
                ValueKind::Other
            }
        }
        Type::Path(path) => match path.path.segments.last().map(|item| item.ident.to_string()) {
            Some(name) if name == "Timestamp" => ValueKind::Timestamp,
            Some(name) if name == "EventId" => ValueKind::EventId,
            _ => ValueKind::Other,
        },
        _ => ValueKind::Other,
    }
}

fn parameter_kinds(
    inputs: &syn::punctuated::Punctuated<FnArg, syn::token::Comma>,
    _output: &ReturnType,
) -> BTreeMap<String, ValueKind> {
    inputs
        .iter()
        .filter_map(|argument| match argument {
            FnArg::Typed(argument) => match &*argument.pat {
                Pat::Ident(binding) => Some((binding.ident.to_string(), type_kind(&argument.ty))),
                _ => None,
            },
            FnArg::Receiver(_) => None,
        })
        .collect()
}

fn ordered_parameter_kinds(
    inputs: &syn::punctuated::Punctuated<FnArg, syn::token::Comma>,
) -> Vec<ValueKind> {
    inputs
        .iter()
        .map(|argument| match argument {
            FnArg::Typed(argument) => type_kind(&argument.ty),
            FnArg::Receiver(_) => ValueKind::Other,
        })
        .collect()
}

#[derive(Clone)]
enum CallTarget {
    Free {
        module: Option<String>,
        name: String,
        arity: Option<usize>,
        argument_kinds: Option<Vec<ValueKind>>,
    },
    Method {
        module: Option<String>,
        owner: String,
        name: String,
        arity: Option<usize>,
        argument_kinds: Option<Vec<ValueKind>>,
    },
    Closure(ExprClosure),
}

impl CallTarget {
    fn with_call(self, argument_kinds: Vec<ValueKind>) -> Self {
        let called_arity = argument_kinds.len();
        match self {
            Self::Free { module, name, .. } => Self::Free {
                module,
                name,
                arity: Some(called_arity),
                argument_kinds: Some(argument_kinds),
            },
            Self::Method {
                module,
                owner,
                name,
                ..
            } => Self::Method {
                module,
                owner,
                name,
                arity: Some(called_arity),
                argument_kinds: Some(argument_kinds),
            },
            Self::Closure(closure) => Self::Closure(closure),
        }
    }

    fn name(&self) -> Option<&str> {
        match self {
            Self::Free { name, .. } | Self::Method { name, .. } => Some(name),
            Self::Closure(_) => None,
        }
    }
}

#[derive(Default)]
struct ImportAliases {
    modules: Vec<String>,
    values: BTreeMap<String, BTreeMap<String, CallTarget>>,
}

impl ImportAliases {
    fn new(path: &str) -> Self {
        Self {
            modules: source_module(path),
            values: BTreeMap::new(),
        }
    }

    fn module(&self) -> String {
        self.modules.join("::")
    }

    fn collect(&mut self, prefix: &mut Vec<String>, tree: &UseTree) {
        match tree {
            UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.collect(prefix, &path.tree);
                prefix.pop();
            }
            UseTree::Name(name) => {
                let original = name.ident.to_string();
                self.insert(prefix, &original, original.clone());
            }
            UseTree::Rename(rename) => {
                self.insert(prefix, &rename.ident.to_string(), rename.rename.to_string());
            }
            UseTree::Group(group) => {
                for item in &group.items {
                    self.collect(prefix, item);
                }
            }
            UseTree::Glob(_) => {}
        }
    }

    fn insert(&mut self, prefix: &[String], original: &str, exposed: String) {
        let target = prefix.last().map_or_else(
            || CallTarget::Free {
                module: None,
                name: original.to_owned(),
                arity: None,
                argument_kinds: None,
            },
            |qualifier| {
                if qualifier.chars().next().is_some_and(char::is_uppercase) {
                    CallTarget::Method {
                        module: (!prefix[..prefix.len() - 1].is_empty())
                            .then(|| prefix[..prefix.len() - 1].join("::")),
                        owner: qualifier.clone(),
                        name: original.to_owned(),
                        arity: None,
                        argument_kinds: None,
                    }
                } else {
                    CallTarget::Free {
                        module: Some(prefix.join("::")),
                        name: original.to_owned(),
                        arity: None,
                        argument_kinds: None,
                    }
                }
            },
        );
        self.values
            .entry(self.module())
            .or_default()
            .insert(exposed, target);
    }
}

impl<'ast> Visit<'ast> for ImportAliases {
    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        if let Some((_, items)) = &node.content {
            self.modules.push(node.ident.to_string());
            for item in items {
                self.visit_item(item);
            }
            self.modules.pop();
        }
    }

    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        self.collect(&mut Vec::new(), &node.tree);
    }
}

struct Aliases {
    owner: Option<String>,
    values: BTreeMap<String, CallTarget>,
}

impl Aliases {
    fn new(owner: Option<String>, imported: BTreeMap<String, CallTarget>) -> Self {
        Self {
            owner,
            values: imported,
        }
    }

    fn target(&self, expr: &Expr) -> Option<CallTarget> {
        match expr {
            Expr::Closure(closure) => Some(CallTarget::Closure(closure.clone())),
            Expr::Path(path) => {
                let segments = path.path.segments.iter().collect::<Vec<_>>();
                let last = segments.last()?.ident.to_string();
                if segments.len() == 1 {
                    return Some(CallTarget::Free {
                        module: None,
                        name: last,
                        arity: None,
                        argument_kinds: None,
                    });
                }
                let qualifier = segments.get(segments.len() - 2)?.ident.to_string();
                if qualifier == "Self" {
                    self.owner.clone().map(|owner| CallTarget::Method {
                        module: None,
                        owner,
                        name: last,
                        arity: None,
                        argument_kinds: None,
                    })
                } else if qualifier.chars().next().is_some_and(char::is_uppercase) {
                    Some(CallTarget::Method {
                        module: (segments.len() > 2).then(|| {
                            segments[..segments.len() - 2]
                                .iter()
                                .map(|segment| segment.ident.to_string())
                                .collect::<Vec<_>>()
                                .join("::")
                        }),
                        owner: qualifier,
                        name: last,
                        arity: None,
                        argument_kinds: None,
                    })
                } else {
                    Some(CallTarget::Free {
                        module: Some(
                            segments[..segments.len() - 1]
                                .iter()
                                .map(|segment| segment.ident.to_string())
                                .collect::<Vec<_>>()
                                .join("::"),
                        ),
                        name: last,
                        arity: None,
                        argument_kinds: None,
                    })
                }
            }
            _ => None,
        }
    }
}

impl<'ast> Visit<'ast> for Aliases {
    fn visit_local(&mut self, node: &'ast Local) {
        if let Pat::Ident(binding) = &node.pat
            && let Some(initializer) = &node.init
            && let Some(target) = self.target(&initializer.expr)
        {
            self.values.insert(binding.ident.to_string(), target);
        }
        visit::visit_local(self, node);
    }
}

struct ValueKinds {
    values: BTreeMap<String, ValueKind>,
}

impl ValueKinds {
    fn new(parameters: &BTreeMap<String, ValueKind>) -> Self {
        Self {
            values: parameters.clone(),
        }
    }

    fn expr_kind(&self, expr: &Expr) -> ValueKind {
        match expr {
            Expr::Paren(paren) => self.expr_kind(&paren.expr),
            Expr::Reference(reference) => self.expr_kind(&reference.expr),
            Expr::Path(path) if path.path.segments.len() == 1 => self
                .values
                .get(&path.path.segments[0].ident.to_string())
                .copied()
                .unwrap_or(ValueKind::Other),
            Expr::Field(field) => match &field.member {
                syn::Member::Named(name) if name == "created_at" => ValueKind::Timestamp,
                syn::Member::Named(name) if name == "id" || name == "event_id" => {
                    ValueKind::EventId
                }
                _ => ValueKind::Other,
            },
            Expr::MethodCall(call) if call.method == "created_at" => ValueKind::Timestamp,
            Expr::MethodCall(call) if call.method == "id" => ValueKind::EventId,
            Expr::Tuple(tuple) if tuple.elems.len() == 2 => {
                let mut values = tuple.elems.iter();
                if values
                    .next()
                    .is_some_and(|item| self.expr_kind(item) == ValueKind::Timestamp)
                    && values
                        .next()
                        .is_some_and(|item| self.expr_kind(item) == ValueKind::EventId)
                {
                    ValueKind::EventOrderKey
                } else {
                    ValueKind::Other
                }
            }
            _ => ValueKind::Other,
        }
    }
}

impl<'ast> Visit<'ast> for ValueKinds {
    fn visit_local(&mut self, node: &'ast Local) {
        if let Pat::Ident(binding) = &node.pat
            && let Some(initializer) = &node.init
        {
            let kind = self.expr_kind(&initializer.expr);
            if kind != ValueKind::Other {
                self.values.insert(binding.ident.to_string(), kind);
            }
        }
        visit::visit_local(self, node);
    }

    fn visit_expr_closure(&mut self, node: &'ast ExprClosure) {
        for input in &node.inputs {
            if let Pat::Type(typed) = input
                && let Pat::Ident(binding) = &*typed.pat
            {
                let kind = type_kind(&typed.ty);
                if kind != ValueKind::Other {
                    self.values.insert(binding.ident.to_string(), kind);
                }
            }
        }
        self.visit_expr(&node.body);
    }
}

fn is_raw_event_ordering(expr: &Expr, kinds: &ValueKinds) -> bool {
    let (left, right) = match expr {
        Expr::Binary(binary)
            if matches!(
                binary.op,
                syn::BinOp::Lt(_) | syn::BinOp::Le(_) | syn::BinOp::Gt(_) | syn::BinOp::Ge(_)
            ) =>
        {
            (&*binary.left, &*binary.right)
        }
        Expr::MethodCall(call)
            if matches!(
                call.method.to_string().as_str(),
                "cmp" | "partial_cmp" | "lt" | "le" | "gt" | "ge" | "max" | "min"
            ) && call.args.first().is_some() =>
        {
            (&*call.receiver, &call.args[0])
        }
        _ => return false,
    };
    let left = kinds.expr_kind(left);
    left != ValueKind::Other && left == kinds.expr_kind(right)
}

fn call_target_name(
    path: &syn::ExprPath,
    aliases: &BTreeMap<String, CallTarget>,
) -> Option<String> {
    let name = path.path.segments.last()?.ident.to_string();
    if path.path.segments.len() == 1 {
        aliases
            .get(&name)
            .and_then(CallTarget::name)
            .map_or(Some(name), |target| Some(target.to_owned()))
    } else {
        Some(name)
    }
}

fn is_raw_event_ordering_call(
    call: &syn::ExprCall,
    aliases: &BTreeMap<String, CallTarget>,
    kinds: &ValueKinds,
) -> bool {
    let Expr::Path(path) = &*call.func else {
        return false;
    };
    let Some(name) = call_target_name(path, aliases) else {
        return false;
    };
    if !matches!(
        name.as_str(),
        "cmp" | "partial_cmp" | "lt" | "le" | "gt" | "ge" | "max" | "min"
    ) {
        return false;
    }
    let mut arguments = call.args.iter();
    let (Some(left), Some(right)) = (arguments.next(), arguments.next()) else {
        return false;
    };
    let left = kinds.expr_kind(left);
    left != ValueKind::Other && left == kinds.expr_kind(right)
}

#[derive(Default)]
struct ComparatorUse {
    live_owner_calls: usize,
    controlled_owner_sinks: BTreeSet<String>,
    raw_ordering: Vec<String>,
    calls: Vec<CallTarget>,
}

struct OwnerCallFinder<'a> {
    aliases: &'a BTreeMap<String, CallTarget>,
    found: bool,
    resolving: BTreeSet<String>,
}

impl OwnerCallFinder<'_> {
    fn target(&mut self, target: CallTarget) {
        match target {
            CallTarget::Free { name, .. } if name == "event_is_newer" => self.found = true,
            CallTarget::Free { name, .. } if self.aliases.contains_key(&name) => {
                if self.resolving.insert(name.clone()) {
                    self.target(self.aliases[&name].clone());
                    self.resolving.remove(&name);
                }
            }
            CallTarget::Closure(closure) => self.visit_expr(&closure.body),
            CallTarget::Free { .. } | CallTarget::Method { .. } => {}
        }
    }
}

impl<'ast> Visit<'ast> for OwnerCallFinder<'_> {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Expr::Path(path) = &*node.func {
            let name = path
                .path
                .segments
                .last()
                .expect("called path has a segment")
                .ident
                .to_string();
            self.target(CallTarget::Free {
                module: None,
                name,
                arity: Some(node.args.len()),
                argument_kinds: None,
            });
        }
        visit::visit_expr_call(self, node);
    }
}

struct SinkFinder {
    sinks: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for SinkFinder {
    fn visit_expr_assign(&mut self, node: &'ast syn::ExprAssign) {
        if matches!(&*node.left, Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_))) {
            self.sinks.insert(normalize(node));
        }
        visit::visit_expr_assign(self, node);
    }

    fn visit_expr_return(&mut self, node: &'ast syn::ExprReturn) {
        visit::visit_expr_return(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if matches!(&*node.func, Expr::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "Ok" || segment.ident == "Err" || segment.ident == "incoherent"))
        {
            self.sinks.insert(normalize(node));
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        if node.method == "insert" {
            self.sinks.insert(normalize(node));
        }
        visit::visit_expr_method_call(self, node);
    }
}

struct ComparatorVisitor<'a> {
    owner: Option<String>,
    aliases: &'a BTreeMap<String, CallTarget>,
    kinds: &'a ValueKinds,
    use_: ComparatorUse,
    resolving_aliases: BTreeSet<String>,
    owner_predicates: BTreeSet<String>,
    dead: usize,
}

impl<'a> ComparatorVisitor<'a> {
    fn new(
        owner: Option<String>,
        aliases: &'a BTreeMap<String, CallTarget>,
        kinds: &'a ValueKinds,
    ) -> Self {
        Self {
            owner,
            aliases,
            kinds,
            use_: ComparatorUse::default(),
            resolving_aliases: BTreeSet::new(),
            owner_predicates: BTreeSet::new(),
            dead: 0,
        }
    }

    fn contains_owner_call(&self, expr: &Expr) -> bool {
        if matches!(expr, Expr::Path(path) if path.path.segments.len() == 1 && self.owner_predicates.contains(&path.path.segments[0].ident.to_string()))
        {
            return true;
        }
        let mut finder = OwnerCallFinder {
            aliases: self.aliases,
            found: false,
            resolving: BTreeSet::new(),
        };
        finder.visit_expr(expr);
        finder.found
    }

    fn sinks_in_expr(expr: &Expr) -> BTreeSet<String> {
        let mut finder = SinkFinder {
            sinks: BTreeSet::new(),
        };
        finder.visit_expr(expr);
        finder.sinks
    }

    fn sinks_in_block(block: &Block) -> BTreeSet<String> {
        let mut finder = SinkFinder {
            sinks: BTreeSet::new(),
        };
        finder.visit_block(block);
        finder.sinks
    }

    fn record_controlled_sinks(
        &mut self,
        controller: &Expr,
        branch: &str,
        sinks: BTreeSet<String>,
    ) {
        let controller = normalize(controller);
        self.use_.controlled_owner_sinks.extend(
            sinks
                .into_iter()
                .map(|sink| format!("{controller}:{branch}:{sink}")),
        );
    }

    fn record_target(&mut self, target: CallTarget) {
        match target {
            CallTarget::Free { name, .. } if name == "event_is_newer" => {
                if self.dead == 0 {
                    self.use_.live_owner_calls += 1;
                }
            }
            CallTarget::Free {
                name,
                arity,
                argument_kinds,
                ..
            } if self.aliases.contains_key(&name) => {
                if matches!(&self.aliases[&name], CallTarget::Free { name: original, .. } if original == &name)
                {
                    self.use_.calls.push(CallTarget::Free {
                        module: None,
                        name,
                        arity,
                        argument_kinds,
                    });
                    return;
                }
                if self.resolving_aliases.insert(name.clone()) {
                    let target = self.aliases[&name]
                        .clone()
                        .with_call(argument_kinds.expect("called alias has exact argument kinds"));
                    self.record_target(target);
                    self.resolving_aliases.remove(&name);
                }
            }
            CallTarget::Closure(closure) => self.visit_expr(&closure.body),
            target => self.use_.calls.push(target),
        }
    }

    fn path_target(&self, path: &syn::ExprPath) -> Option<CallTarget> {
        let segments = path.path.segments.iter().collect::<Vec<_>>();
        let last = segments.last()?.ident.to_string();
        if segments.len() == 1 {
            return Some(CallTarget::Free {
                module: None,
                name: last,
                arity: None,
                argument_kinds: None,
            });
        }
        let qualifier = segments.get(segments.len() - 2)?.ident.to_string();
        if qualifier == "Self" {
            self.owner.clone().map(|owner| CallTarget::Method {
                module: None,
                owner,
                name: last,
                arity: None,
                argument_kinds: None,
            })
        } else if qualifier.chars().next().is_some_and(char::is_uppercase) {
            Some(CallTarget::Method {
                module: (segments.len() > 2).then(|| {
                    segments[..segments.len() - 2]
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect::<Vec<_>>()
                        .join("::")
                }),
                owner: qualifier,
                name: last,
                arity: None,
                argument_kinds: None,
            })
        } else {
            Some(CallTarget::Free {
                module: Some(
                    segments[..segments.len() - 1]
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect::<Vec<_>>()
                        .join("::"),
                ),
                name: last,
                arity: None,
                argument_kinds: None,
            })
        }
    }
}

impl<'ast> Visit<'ast> for ComparatorVisitor<'_> {
    fn visit_expr_if(&mut self, node: &'ast ExprIf) {
        if self.dead == 0 && self.contains_owner_call(&node.cond) {
            self.record_controlled_sinks(
                &node.cond,
                "then",
                Self::sinks_in_block(&node.then_branch),
            );
            if let Some((_, otherwise)) = &node.else_branch {
                self.record_controlled_sinks(&node.cond, "else", Self::sinks_in_expr(otherwise));
            }
        }
        self.visit_expr(&node.cond);
        if matches!(&*node.cond, Expr::Lit(lit) if matches!(&lit.lit, syn::Lit::Bool(value) if !value.value))
        {
            self.dead += 1;
            self.visit_block(&node.then_branch);
            self.dead -= 1;
        } else {
            self.visit_block(&node.then_branch);
        }
        if let Some((_, otherwise)) = &node.else_branch {
            self.visit_expr(otherwise);
        }
    }

    fn visit_local(&mut self, node: &'ast Local) {
        if let Pat::Ident(binding) = &node.pat
            && let Some(initializer) = &node.init
            && self.contains_owner_call(&initializer.expr)
        {
            self.owner_predicates.insert(binding.ident.to_string());
        }
        if let Some(initializer) = &node.init
            && !matches!(&*initializer.expr, Expr::Closure(_) | Expr::Path(_))
        {
            self.visit_expr(&initializer.expr);
        }
        if let Some(initializer) = &node.init
            && let Some((_, diverge)) = &initializer.diverge
        {
            self.visit_expr(diverge);
        }
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if self.dead == 0 && is_raw_event_ordering_call(node, self.aliases, self.kinds) {
            self.use_.raw_ordering.push(normalize(node));
        }
        if let Expr::Path(path) = &*node.func
            && let Some(target) = self.path_target(path)
        {
            self.record_target(
                target.with_call(
                    node.args
                        .iter()
                        .map(|argument| self.kinds.expr_kind(argument))
                        .collect(),
                ),
            );
        }
        for argument in &node.args {
            self.visit_expr(argument);
        }
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        let expression = Expr::Binary(node.clone());
        if self.dead == 0 && is_raw_event_ordering(&expression, self.kinds) {
            self.use_.raw_ordering.push(normalize(node));
        }
        visit::visit_expr_binary(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        let method = node.method.to_string();
        let expression = Expr::MethodCall(node.clone());
        if self.dead == 0 && is_raw_event_ordering(&expression, self.kinds) {
            self.use_.raw_ordering.push(normalize(node));
        }
        if matches!(&*node.receiver, Expr::Path(path) if path.path.is_ident("self"))
            && let Some(owner) = &self.owner
        {
            self.use_.calls.push(CallTarget::Method {
                module: None,
                owner: owner.clone(),
                name: method,
                arity: Some(node.args.len() + 1),
                argument_kinds: Some(
                    std::iter::once(self.kinds.expr_kind(&node.receiver))
                        .chain(
                            node.args
                                .iter()
                                .map(|argument| self.kinds.expr_kind(argument)),
                        )
                        .collect(),
                ),
            });
        }
        self.visit_expr(&node.receiver);
        for argument in &node.args {
            self.visit_expr(argument);
        }
    }

    fn visit_expr_match(&mut self, node: &'ast ExprMatch) {
        if self.dead == 0 {
            for arm in &node.arms {
                if arm
                    .guard
                    .as_ref()
                    .is_some_and(|(_, guard)| self.contains_owner_call(guard))
                {
                    let guard = &arm.guard.as_ref().expect("guard checked").1;
                    self.record_controlled_sinks(guard, "arm", Self::sinks_in_expr(&arm.body));
                }
            }
        }
        visit::visit_expr_match(self, node);
    }
}

struct Corpus {
    files: BTreeSet<String>,
    functions: Vec<Function>,
    imports: BTreeMap<(String, String), BTreeMap<String, CallTarget>>,
}

impl Corpus {
    fn from_root(root: &Path) -> Self {
        let files = rust_files(root);
        let mut functions = Vec::new();
        let mut imports = BTreeMap::new();
        for relative in &files {
            let source = fs::read_to_string(root.join(relative))
                .unwrap_or_else(|error| panic!("failed to read {relative}: {error}"));
            let file = syn::parse_file(&source)
                .unwrap_or_else(|error| panic!("failed to parse {relative}: {error}"));
            let mut found = Functions::new(relative.clone());
            found.visit_file(&file);
            functions.extend(found.values);
            let mut aliases = ImportAliases::new(relative);
            aliases.visit_file(&file);
            imports.extend(
                aliases
                    .values
                    .into_iter()
                    .map(|(module, values)| ((relative.clone(), module), values)),
            );
        }
        Self {
            files,
            functions,
            imports,
        }
    }

    fn parse_fixture(files: &[(&str, &str)]) -> Self {
        let mut paths = BTreeSet::new();
        let mut functions = Vec::new();
        let mut imports = BTreeMap::new();
        for (path, source) in files {
            let file = syn::parse_file(source)
                .unwrap_or_else(|error| panic!("failed to parse fixture {path}: {error}"));
            let mut found = Functions::new((*path).to_owned());
            found.visit_file(&file);
            paths.insert((*path).to_owned());
            functions.extend(found.values);
            let mut aliases = ImportAliases::new(path);
            aliases.visit_file(&file);
            imports.extend(
                aliases
                    .values
                    .into_iter()
                    .map(|(module, values)| (((*path).to_owned(), module), values)),
            );
        }
        Self {
            files: paths,
            functions,
            imports,
        }
    }

    fn exact(&self, expected: &FunctionId) -> &Function {
        let matches = self
            .functions
            .iter()
            .filter(|function| function.id == *expected)
            .collect::<Vec<_>>();
        assert_eq!(
            matches.len(),
            1,
            "expected exactly one manifest function {expected:?}, found {}",
            matches.len()
        );
        matches[0]
    }

    fn resolve(&self, caller: &FunctionId, target: &CallTarget) -> Vec<&Function> {
        let crate_path = caller.path.split('/').take(2).collect::<Vec<_>>().join("/");
        let candidates = match target {
            CallTarget::Free {
                module,
                name,
                arity,
                argument_kinds,
            } => self
                .functions
                .iter()
                .filter(|function| {
                    function.id.owner.is_none()
                        && function.id.name == *name
                        && call_signature_matches(function, *arity, argument_kinds.as_deref())
                        && module.as_ref().is_none_or(|target_module| {
                            function.id.module == resolve_module(&caller.module, target_module)
                        })
                })
                .collect::<Vec<_>>(),
            CallTarget::Method {
                module,
                owner,
                name,
                arity,
                argument_kinds,
            } => self
                .functions
                .iter()
                .filter(|function| {
                    function.id.name == *name
                        && call_signature_matches(function, *arity, argument_kinds.as_deref())
                        && module.as_ref().is_none_or(|target_module| {
                            function.id.module == resolve_module(&caller.module, target_module)
                        })
                        && function.id.owner.as_ref().is_some_and(|candidate| {
                            candidate == owner || candidate.ends_with(&format!("::{owner}"))
                        })
                })
                .collect::<Vec<_>>(),
            CallTarget::Closure(_) => Vec::new(),
        };
        let same_module = candidates
            .iter()
            .copied()
            .filter(|function| function.id.module == caller.module)
            .collect::<Vec<_>>();
        if !same_module.is_empty() {
            return same_module;
        }
        candidates
            .into_iter()
            .filter(|function| function.id.path.starts_with(&format!("{crate_path}/")))
            .collect()
    }
}

fn call_signature_matches(
    function: &Function,
    arity: Option<usize>,
    arguments: Option<&[ValueKind]>,
) -> bool {
    arity.is_none_or(|called| function.arity == called)
        && arguments.is_none_or(|called| {
            function
                .parameter_order
                .iter()
                .zip(called)
                .all(|(parameter, argument)| {
                    *parameter == ValueKind::Other
                        || *argument == ValueKind::Other
                        || parameter == argument
                })
        })
}

fn normalize(tokens: &impl ToTokens) -> String {
    tokens
        .to_token_stream()
        .to_string()
        .split_whitespace()
        .collect()
}

fn source_module(path: &str) -> Vec<String> {
    let Some((_, relative)) = path.split_once("/src/") else {
        return Vec::new();
    };
    let mut parts = relative.split('/').map(str::to_owned).collect::<Vec<_>>();
    let Some(file) = parts.pop() else {
        return Vec::new();
    };
    let stem = file.strip_suffix(".rs").unwrap_or(&file);
    if !matches!(stem, "lib" | "main" | "mod") {
        parts.push(stem.to_owned());
    }
    parts
}

fn resolve_module(caller: &str, target: &str) -> String {
    let mut target = target.split("::").collect::<Vec<_>>();
    if target.first() == Some(&"crate") {
        target.remove(0);
        return target.join("::");
    }
    let mut module = caller
        .split("::")
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if target.first() == Some(&"self") {
        target.remove(0);
        module.extend(target);
        return module.join("::");
    }
    while target.first() == Some(&"super") {
        target.remove(0);
        module.pop();
    }
    if target.is_empty() {
        module.join("::")
    } else {
        target.join("::")
    }
}

fn manifest_dir() -> PathBuf {
    match (
        std::env::var_os("TEST_SRCDIR"),
        std::env::var_os("TEST_WORKSPACE"),
    ) {
        (Some(source), Some(workspace)) => {
            Path::new(&source).join(workspace).join("crates/fava-state")
        }
        _ => PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    }
}

fn repository_root() -> PathBuf {
    manifest_dir()
        .parent()
        .and_then(Path::parent)
        .expect("fava-state is beneath crates/ at the repository root")
        .to_path_buf()
}

fn rust_files(root: &Path) -> BTreeSet<String> {
    fn visit(root: &Path, directory: &Path, output: &mut BTreeSet<String>) {
        let mut entries = fs::read_dir(directory)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_else(|error| panic!("failed to enumerate {}: {error}", directory.display()));
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, output);
            } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
                output.insert(
                    path.strip_prefix(root)
                        .expect("repository-contained Rust file")
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }

    let mut output = BTreeSet::new();
    for scope in ["crates", "apps", "falsifiers"] {
        let directory = root.join(scope);
        if directory.exists() {
            visit(root, &directory, &mut output);
        }
    }
    output
}

fn workspace_members(root: &Path) -> Vec<String> {
    let manifest =
        fs::read_to_string(root.join("Cargo.toml")).expect("root Cargo.toml is readable");
    let mut members = Vec::new();
    let mut in_members = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed == "members = [" {
            in_members = true;
        } else if in_members && trimmed == "]" {
            break;
        } else if in_members {
            let member = trimmed.trim_end_matches(',').trim_matches('"');
            if !member.is_empty() {
                members.push(member.to_owned());
            }
        }
    }
    assert!(
        !members.is_empty(),
        "workspace members must be discoverable"
    );
    members
}

fn direct(function: &Function, corpus: &Corpus) -> ComparatorUse {
    let imported = corpus
        .imports
        .get(&(function.id.path.clone(), function.id.module.clone()))
        .cloned()
        .unwrap_or_default();
    let mut aliases = Aliases::new(function.id.owner.clone(), imported);
    aliases.visit_block(&function.block);
    let mut kinds = ValueKinds::new(&function.parameters);
    kinds.visit_block(&function.block);
    let mut visitor = ComparatorVisitor::new(function.id.owner.clone(), &aliases.values, &kinds);
    visitor.visit_block(&function.block);
    visitor.use_
}

fn reachable(root: &Function, corpus: &Corpus) -> (ComparatorUse, BTreeSet<FunctionId>) {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    let mut total = ComparatorUse::default();
    while let Some(function) = pending.pop() {
        if !visited.insert(function.id.clone()) {
            continue;
        }
        let use_ = direct(function, corpus);
        total.live_owner_calls += use_.live_owner_calls;
        total
            .controlled_owner_sinks
            .extend(use_.controlled_owner_sinks);
        total.raw_ordering.extend(use_.raw_ordering);
        for target in use_.calls {
            pending.extend(corpus.resolve(&function.id, &target));
        }
    }
    (total, visited)
}

fn expected_manifest() -> BTreeSet<FunctionId> {
    [
        ("crates/fava-query-standard/src/lib.rs", "insert_newest", None, "fninsert_newest<K:Ord>(records:&mutBTreeMap<K,EventRecord>,key:K,incoming:EventRecord)"),
        ("crates/fava-publication/src/materialization.rs", "semantic_successor", Some("Publication"), "fnsemantic_successor(&self,state:&SemanticState,receipt_id:ReceiptId,)->Result<(bool,Option<EventValue>),PublicationError>"),
        ("crates/fava-write-store-memory/src/semantic.rs", "install_semantic", Some("MemoryWriteStore"), "fninstall_semantic(&self,write_id:WriteId,receipt_id:ReceiptId,expected:MaterializationId,expected_source:Option<EventId>,applied_edits:&[ReplaceableEventEdit],event:UnsignedEvent,source:Option<&EventValue>,initial_route:Option<&RoutePlan>,)->Result<Receipt,WriteStoreError>"),
        ("crates/fava-write-store-memory/src/semantic_acceptance.rs", "validate_materialization", None, "fnvalidate_materialization(edit:&ReplaceableEventEdit,author:PublicKey,event:&UnsignedEvent,source:Option<&EventValue>,routing:&WriteRouting,)->Result<Option<(EventId,Timestamp)>,WriteStoreError>"),
        ("crates/fava-write-store-redb/src/semantic.rs", "install_semantic", Some("RedbWriteStore"), "fninstall_semantic(&self,write_id:WriteId,receipt_id:ReceiptId,expected:MaterializationId,expected_source:Option<EventId>,applied_edits:&[ReplaceableEventEdit],event:UnsignedEvent,source:Option<&EventValue>,initial_route:Option<&RoutePlan>,)->Result<Receipt,WriteStoreError>"),
        ("crates/fava-write-store-redb/src/semantic_acceptance.rs", "validate_materialization", None, "fnvalidate_materialization(edit:&ReplaceableEventEdit,author:PublicKey,event:&UnsignedEvent,source:Option<&EventValue>,routing:&WriteRouting,)->Result<Option<(EventId,Timestamp)>,WriteStoreError>"),
        ("crates/fava-write-store-redb/src/validation.rs", "validate_semantic", None, "fnvalidate_semantic(receipt:&Receipt,(edits,author,current_source,failed_source,successor):&SemanticCustody,)->Result<(),WriteStoreError>"),
        ("apps/canary/src/croissant_simple_groups_evidence_semantics/value_support.rs", "select_current", None, "fnselect_current(current:&mutOption<Event>,candidate:Event)"),
    ]
    .into_iter()
    .map(|(path, name, owner, signature)| FunctionId {
        path: path.to_owned(),
        module: source_module(path).join("::"),
        owner: owner.map(str::to_owned),
        name: name.to_owned(),
        signature: signature.to_owned(),
    })
    .collect()
}

fn expected_non_winner_ordering_manifest() -> BTreeMap<FunctionId, BTreeSet<String>> {
    // These are the complete arbitrary Timestamp/EventId-shaped comparisons
    // outside fava-state that do not select same-coordinate event state:
    // protocol edits require a strictly later unsigned materialization time;
    // query evaluation orders already-selected output; subscription code orders
    // subscription/demand identities whose accessor happens to be named `id`.
    [
        (
            ("crates/fava-bookmarks/src/lib.rs", "qualified_source", None, "fnqualified_source(author:PublicKey,source:Option<&EventValue>,created_at:Timestamp,)->Result<(&str,&[Tag]),WriteIntentError>"),
            &["created_at<=source.created_at()"][..],
        ),
        (
            ("crates/fava-nip02/src/edit.rs", "qualified_source", None, "fnqualified_source(author:PublicKey,source:Option<&EventValue>,created_at:Timestamp,)->Result<(&str,&[Tag]),WriteIntentError>"),
            &["created_at<=source.created_at()"][..],
        ),
        (
            ("crates/fava-query-standard/src/lib.rs", "evaluate", Some("StandardQueryEvaluator"), "fnevaluate(&self,query:&Query,sources:&[SourceSnapshot],)->Result<QuerySnapshot,QueryEvaluationError>"),
            &[
                "left.created_at().cmp(&right.created_at())",
                "left.id().cmp(&right.id())",
                "right.created_at().cmp(&left.created_at())",
                "right.id().cmp(&left.id())",
            ][..],
        ),
        (
            ("crates/fava-simple-groups/src/edit.rs", "qualified_source", None, "fnqualified_source(author:PublicKey,source:Option<&EventValue>,created_at:Timestamp,)->Result<(&str,&[Tag]),WriteIntentError>"),
            &["created_at<=source.created_at()"][..],
        ),
        (
            ("crates/fava-subscriptions-standard/src/diff.rs", "assemble", None, "fnassemble(relay:&RelaySessionKey,revision:PlanRevision,opened:Vec<(SubscriptionId,AttributedSubscription)>,constraints:&RelayReadConstraints,installed:&InstalledSubscriptions,owners:&BTreeMap<SubscriptionId,BTreeSet<DemandId>>,shortfalls:Vec<SubscriptionShortfall>,)->SubscriptionPlan"),
            &["left.id.cmp(&right.id)"][..],
        ),
        (
            ("crates/fava-subscriptions-standard/src/grouping.rs", "canonical_order", None, "fncanonical_order(demand:&[RelayDemand])->Vec<RelayDemand>"),
            &["left.1.id().cmp(&right.1.id())"][..],
        ),
    ]
    .into_iter()
    .map(|((path, name, owner, signature), expressions)| {
        (
            FunctionId {
                path: path.to_owned(),
                module: source_module(path).join("::"),
                owner: owner.map(str::to_owned),
                name: name.to_owned(),
                signature: signature.to_owned(),
            },
            expressions.iter().map(|value| (*value).to_owned()).collect(),
        )
    })
    .collect()
}

#[allow(
    clippy::too_many_lines,
    reason = "the exact sink allowlist is intentionally one auditable manifest"
)]
fn expected_controlled_sink_manifest() -> BTreeMap<FunctionId, BTreeSet<String>> {
    [
        (
            (
                "apps/canary/src/croissant_simple_groups_evidence_semantics/value_support.rs",
                "select_current",
                None,
                "fnselect_current(current:&mutOption<Event>,candidate:Event)",
            ),
            &[r"replace:then:*current=Some(candidate)"][..],
        ),
        (
            (
                "crates/fava-publication/src/materialization.rs",
                "semantic_successor",
                Some("Publication"),
                "fnsemantic_successor(&self,state:&SemanticState,receipt_id:ReceiptId,)->Result<(bool,Option<EventValue>),PublicationError>",
            ),
            &[r"state.source_floor.is_none_or(|floor|{candidate.id().is_some_and(|candidate_id|{event_is_newer((candidate.created_at(),candidate_id),(floor,state.selected_id.unwrap_or(candidate_id)),)})}):arm:Ok((true,Some(candidate)))"][..],
        ),
        (
            (
                "crates/fava-query-standard/src/lib.rs",
                "insert_newest",
                None,
                "fninsert_newest<K:Ord>(records:&mutBTreeMap<K,EventRecord>,key:K,incoming:EventRecord)",
            ),
            &[r"event_is_newer((incoming.created_at(),incoming.id()),(entry.get().created_at(),entry.get().id()),):then:entry.insert(incoming)"][..],
        ),
        (
            (
                "crates/fava-write-store-memory/src/semantic_acceptance.rs",
                "validate_materialization",
                None,
                "fnvalidate_materialization(edit:&ReplaceableEventEdit,author:PublicKey,event:&UnsignedEvent,source:Option<&EventValue>,routing:&WriteRouting,)->Result<Option<(EventId,Timestamp)>,WriteStoreError>",
            ),
            &[r#"!event_is_newer((event.created_at,event_id),(source_time,source_id)):then:Err(WriteStoreError::Refused("materializationisnotnewerthanitsselectedsource".to_owned(),))"#][..],
        ),
        (
            (
                "crates/fava-write-store-memory/src/semantic.rs",
                "install_semantic",
                Some("MemoryWriteStore"),
                "fninstall_semantic(&self,write_id:WriteId,receipt_id:ReceiptId,expected:MaterializationId,expected_source:Option<EventId>,applied_edits:&[ReplaceableEventEdit],event:UnsignedEvent,source:Option<&EventValue>,initial_route:Option<&RoutePlan>,)->Result<Receipt,WriteStoreError>",
            ),
            &[r#"!event_is_newer((event.created_at,event_id),(receipt.current.event.created_at(),receipt.current.id()),):then:Err(WriteStoreError::Refused("successormaterializationisnotnewerthancurrentevent".to_owned(),))"#][..],
        ),
        (
            (
                "crates/fava-write-store-memory/src/state.rs",
                "require_qualified_source",
                None,
                "fnrequire_qualified_source(current:Option<(EventId,Timestamp)>,candidate:Option<(EventId,Timestamp)>,)->Result<(),WriteStoreError>",
            ),
            &[
                r#"qualified:else:Err(WriteStoreError::Refused("sourceeventisequal,older,oralreadyconsumed".to_owned(),))"#,
                r"qualified:then:Ok(())",
            ][..],
        ),
        (
            (
                "crates/fava-write-store-redb/src/semantic.rs",
                "install_semantic",
                Some("RedbWriteStore"),
                "fninstall_semantic(&self,write_id:WriteId,receipt_id:ReceiptId,expected:MaterializationId,expected_source:Option<EventId>,applied_edits:&[ReplaceableEventEdit],event:UnsignedEvent,source:Option<&EventValue>,initial_route:Option<&RoutePlan>,)->Result<Receipt,WriteStoreError>",
            ),
            &[r#"!event_is_newer((event.created_at,event_id),(receipt.current.event.created_at(),receipt.current.id()),):then:Err(WriteStoreError::Refused("successormaterializationisnotnewerthancurrentevent".to_owned(),))"#][..],
        ),
        (
            (
                "crates/fava-write-store-redb/src/semantic_acceptance.rs",
                "require_qualified_source",
                None,
                "fnrequire_qualified_source(current:Option<(EventId,Timestamp)>,candidate:Option<(EventId,Timestamp)>,)->Result<(),WriteStoreError>",
            ),
            &[
                r#"qualified:else:Err(WriteStoreError::Refused("sourceeventisequal,older,oralreadyconsumed".to_owned(),))"#,
                r"qualified:then:Ok(())",
            ][..],
        ),
        (
            (
                "crates/fava-write-store-redb/src/semantic_acceptance.rs",
                "validate_materialization",
                None,
                "fnvalidate_materialization(edit:&ReplaceableEventEdit,author:PublicKey,event:&UnsignedEvent,source:Option<&EventValue>,routing:&WriteRouting,)->Result<Option<(EventId,Timestamp)>,WriteStoreError>",
            ),
            &[r#"selected.is_some_and(|(source_id,source_time)|{!event_is_newer((event.created_at,event_id),(source_time,source_id))}):then:Err(WriteStoreError::Refused("materializationisnotnewerthanitsselectedsource".to_owned(),))"#][..],
        ),
        (
            (
                "crates/fava-write-store-redb/src/validation.rs",
                "validate_semantic",
                None,
                "fnvalidate_semantic(receipt:&Receipt,(edits,author,current_source,failed_source,successor):&SemanticCustody,)->Result<(),WriteStoreError>",
            ),
            &[r#"receipt.current.event.author()!=*author||receipt.current.event.coordinate().map_err(|error|WriteStoreError::Refused(error.to_string()))?!=crate::semantic::edit_coordinate(edit,*author)||receipt.current.publication.materialization_source!=current_source.map(|(id,_)|id)||current_source.is_some_and(|(id,time)|{!event_is_newer((receipt.current.event.created_at(),receipt.current.id()),(time,id),)}):then:incoherent("durablesemanticcustodyisincoherent")"#][..],
        ),
    ]
    .into_iter()
    .map(|((path, name, owner, signature), sinks)| {
        (
            FunctionId {
                path: path.to_owned(),
                module: source_module(path).join("::"),
                owner: owner.map(str::to_owned),
                name: name.to_owned(),
                signature: signature.to_owned(),
            },
            sinks.iter().map(|sink| (*sink).to_owned()).collect(),
        )
    })
    .collect()
}

fn governed_source(path: &str) -> bool {
    path.contains("/src/") || path.ends_with("/build.rs")
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one repository closure assertion keeps discovery and both exact manifests atomic"
)]
fn repository_discovery_and_exact_manifest_close_every_comparator_path() {
    let root = repository_root();
    let corpus = Corpus::from_root(&root);
    for member in workspace_members(&root) {
        assert!(
            corpus
                .files
                .iter()
                .any(|path| path.starts_with(&format!("{member}/"))),
            "workspace member has no discovered Rust files: {member}"
        );
    }

    let expected = expected_manifest();
    let mut allowed_reachable = BTreeSet::new();
    for id in &expected {
        let function = corpus.exact(id);
        let (use_, reached) = reachable(function, &corpus);
        allowed_reachable.extend(reached);
        assert!(
            use_.live_owner_calls > 0,
            "manifest sink has no reachable live event_is_newer call: {:?}",
            function.id
        );
        assert!(
            !use_.controlled_owner_sinks.is_empty(),
            "manifest owner call does not control insertion/refusal: {:?}",
            function.id
        );
        assert!(
            use_.raw_ordering.is_empty(),
            "manifest sink reaches local ordering {:?}: {:?}",
            function.id,
            use_.raw_ordering
        );
    }

    let unlisted = corpus
        .functions
        .iter()
        .filter(|function| governed_source(&function.id.path))
        .filter(|function| !function.id.path.starts_with("crates/fava-state/"))
        .filter(|function| direct(function, &corpus).live_owner_calls > 0)
        .filter(|function| !allowed_reachable.contains(&function.id))
        .map(|function| function.id.clone())
        .collect::<Vec<_>>();
    assert!(
        unlisted.is_empty(),
        "unmanifested comparator callers discovered in Rust targets: {unlisted:#?}"
    );

    let controlled_sinks = corpus
        .functions
        .iter()
        .filter(|function| governed_source(&function.id.path))
        .filter(|function| !function.id.path.starts_with("crates/fava-state/"))
        .filter_map(|function| {
            let use_ = direct(function, &corpus);
            (!use_.controlled_owner_sinks.is_empty())
                .then(|| (function.id.clone(), use_.controlled_owner_sinks))
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        expected_controlled_sink_manifest(),
        controlled_sinks,
        "exact owner-controlled insertion/refusal sink manifest drifted"
    );

    let raw = corpus
        .functions
        .iter()
        .filter(|function| governed_source(&function.id.path))
        .filter(|function| !function.id.path.starts_with("crates/fava-state/"))
        .filter_map(|function| {
            let use_ = direct(function, &corpus);
            (!use_.raw_ordering.is_empty()).then(|| {
                (
                    function.id.clone(),
                    use_.raw_ordering.into_iter().collect::<BTreeSet<_>>(),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        expected_non_winner_ordering_manifest(),
        raw,
        "arbitrary timestamp/id comparison manifest drifted; same-coordinate winners must delegate to fava-state"
    );

    for path in [
        "crates/fava-nip02/src/contact_list.rs",
        "crates/fava-nip65/src/lib.rs",
    ] {
        assert!(
            corpus.files.contains(path),
            "NoLocalSelection module vanished: {path}"
        );
        let local_selection = corpus
            .functions
            .iter()
            .filter(|function| function.id.path == path)
            .filter_map(|function| {
                let use_ = direct(function, &corpus);
                (use_.live_owner_calls > 0 || !use_.raw_ordering.is_empty())
                    .then(|| function.id.clone())
            })
            .collect::<Vec<_>>();
        assert!(
            local_selection.is_empty(),
            "NoLocalSelection module owns winner choice: {local_selection:#?}"
        );
    }
    assert!(
        !corpus
            .files
            .contains("crates/fava-simple-groups/src/snapshot.rs"),
        "the third NoLocalSelection module was approved for complete subtraction"
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one closure corpus covers cross-file, impl, alias, and self paths"
)]
fn analyzer_rejects_cross_file_impl_helper_and_alias_decoys() {
    let self_helper = Corpus::parse_fixture(&[(
        "crates/example/src/self_helper.rs",
        r"
            struct EventKey { created_at: Timestamp, id: EventId }
            struct Sink;
            impl Sink {
                fn helper(&self, left: EventKey, right: EventKey) -> bool {
                    (left.created_at, left.id) > (right.created_at, right.id)
                }
                fn sink(&self, left: EventKey, right: EventKey) -> bool {
                    event_is_newer((left.created_at, left.id), (right.created_at, right.id))
                        || self.helper(left, right)
                }
            }
        ",
    )]);
    let sink = self_helper
        .functions
        .iter()
        .find(|function| function.id.owner.as_deref() == Some("Sink") && function.id.name == "sink")
        .expect("self.helper fixture sink");
    let (use_, _) = reachable(sink, &self_helper);
    assert_eq!(
        use_.live_owner_calls, 1,
        "one genuine owner call remains live"
    );
    assert!(
        !use_.raw_ordering.is_empty(),
        "self.helper must expose raw ordering"
    );

    let method_alias = Corpus::parse_fixture(&[(
        "crates/example/src/method_alias.rs",
        r"
            struct EventKey { created_at: Timestamp, id: EventId }
            struct Sink;
            impl Sink {
                fn helper(&self, left: EventKey, right: EventKey) -> bool {
                    (left.created_at, left.id) > (right.created_at, right.id)
                }
                fn sink(&self, left: EventKey, right: EventKey) -> bool {
                    let hidden_alias = Self::helper;
                    event_is_newer((left.created_at, left.id), (right.created_at, right.id))
                        || hidden_alias(self, left, right)
                }
            }
        ",
    )]);
    let sink = method_alias
        .functions
        .iter()
        .find(|function| function.id.owner.as_deref() == Some("Sink") && function.id.name == "sink")
        .expect("associated helper alias fixture sink");
    let (use_, _) = reachable(sink, &method_alias);
    assert!(
        !use_.raw_ordering.is_empty(),
        "associated helper alias must expose raw ordering"
    );

    let cross_file = Corpus::parse_fixture(&[
        (
            "crates/example/src/sink.rs",
            r"
            use crate::hidden::cross_file_alias;
            struct EventKey { created_at: Timestamp, id: EventId }
            fn sink(left: EventKey, right: EventKey) -> bool {
                event_is_newer((left.created_at, left.id), (right.created_at, right.id))
                    || cross_file_alias(left, right)
            }
        ",
        ),
        (
            "crates/example/src/hidden.rs",
            r"
            struct EventKey { created_at: Timestamp, id: EventId }
            fn raw_order(left: EventKey, right: EventKey) -> bool {
                (left.created_at, left.id) > (right.created_at, right.id)
            }
            fn cross_file_alias(left: EventKey, right: EventKey) -> bool {
                let hidden_alias = raw_order;
                hidden_alias(left, right)
            }
        ",
        ),
    ]);
    let sink = cross_file
        .functions
        .iter()
        .find(|function| function.id.name == "sink")
        .expect("cross-file fixture sink");
    let (use_, reached) = reachable(sink, &cross_file);
    assert_eq!(
        use_.live_owner_calls, 1,
        "one genuine owner call remains live"
    );
    assert!(
        !use_.raw_ordering.is_empty(),
        "cross-file alias must expose raw ordering"
    );
    assert!(
        reached
            .iter()
            .any(|id| id.path.ends_with("hidden.rs") && id.name == "raw_order"),
        "an unlisted helper file must enter the fixed-point closure"
    );
}

#[test]
fn dead_owner_calls_and_same_name_signature_decoys_do_not_satisfy_a_sink() {
    let corpus = Corpus::parse_fixture(&[(
        "crates/example/src/lib.rs",
        r"
        struct EventKey { created_at: Timestamp, id: EventId }
        fn sink(_: u64, _: u64) -> bool { true }
        fn sink(_: (), _: (), _: ()) -> bool { true }
        fn raw_order(left: EventKey, right: EventKey) -> bool {
            (left.created_at, left.id) > (right.created_at, right.id)
        }
        fn checked(left: EventKey, right: EventKey) -> bool {
            if false {
                return event_is_newer(
                    (left.created_at, left.id),
                    (right.created_at, right.id),
                );
            }
            raw_order(left, right)
        }
    ",
    )]);
    assert_eq!(
        corpus
            .functions
            .iter()
            .filter(|function| function.id.name == "sink")
            .count(),
        2,
        "normalized signatures preserve same-name ambiguity"
    );
    let checked = corpus
        .functions
        .iter()
        .find(|function| function.id.name == "checked")
        .expect("checked fixture");
    let (use_, _) = reachable(checked, &corpus);
    assert_eq!(use_.live_owner_calls, 0, "dead owner calls are not proof");
    assert!(
        !use_.raw_ordering.is_empty(),
        "live raw helper remains visible"
    );
}

#[test]
fn renamed_tuples_alias_and_genuine_owner_call_do_not_hide_a_raw_sink_mutant() {
    let corpus = Corpus::parse_fixture(&[
        (
            "crates/example/src/lib.rs",
            r"
                use fava_state::event_is_newer as approved_order;
                use hidden::renamed_tuple_order as sink_alias;

                fn insertion_sink(
                    proposed: (Timestamp, EventId),
                    installed: (Timestamp, EventId),
                ) -> bool {
                    if sink_alias(proposed, installed) {
                        return true;
                    }
                    let genuine_owner_alias = approved_order;
                    genuine_owner_alias(proposed, installed) && false
                }
            ",
        ),
        (
            "crates/example/src/hidden.rs",
            r"
                fn renamed_tuple_order(
                    first: (Timestamp, EventId),
                    second: (Timestamp, EventId),
                ) -> bool {
                    let renamed_first = first;
                    let renamed_second = second;
                    renamed_first > renamed_second
                }
            ",
        ),
    ]);
    let sink = corpus
        .functions
        .iter()
        .find(|function| function.id.name == "insertion_sink")
        .expect("approved mutant sink");
    let (use_, reached) = reachable(sink, &corpus);
    assert_eq!(
        use_.live_owner_calls, 1,
        "the decoy is a genuine imported and locally aliased owner call"
    );
    assert_eq!(
        use_.controlled_owner_sinks.len(),
        0,
        "the genuine owner call does not control the insertion result"
    );
    assert!(
        !use_.raw_ordering.is_empty(),
        "renamed typed tuples behind an imported helper alias remain discoverable"
    );
    assert!(
        reached
            .iter()
            .any(|id| id.path.ends_with("hidden.rs") && id.name == "renamed_tuple_order"),
        "the imported helper alias must enter the closure"
    );
}

#[test]
fn arbitrary_timestamp_or_id_ordering_is_a_raw_comparator_without_a_compound_key() {
    let corpus = Corpus::parse_fixture(&[(
        "crates/example/src/lib.rs",
        r"
            fn timestamp_only(left: Timestamp, right: Timestamp) -> bool {
                left > right
            }
            fn id_only(left: EventId, right: EventId) -> bool {
                left.lt(&right)
            }
            fn equality_is_not_winner_selection(left: EventId, right: EventId) -> bool {
                left == right
            }
        ",
    )]);
    for name in ["timestamp_only", "id_only"] {
        let function = corpus
            .functions
            .iter()
            .find(|function| function.id.name == name)
            .expect("arbitrary ordering fixture");
        assert!(
            !direct(function, &corpus).raw_ordering.is_empty(),
            "standalone {name} ordering must be rejected"
        );
    }
    let equality = corpus
        .functions
        .iter()
        .find(|function| function.id.name == "equality_is_not_winner_selection")
        .expect("identity equality fixture");
    assert!(
        direct(equality, &corpus).raw_ordering.is_empty(),
        "identity equality alone does not choose a winner"
    );
}

#[test]
fn call_form_ordering_is_governed_like_operator_and_method_ordering() {
    let corpus = Corpus::parse_fixture(&[(
        "crates/example/src/lib.rs",
        r"
            use std::cmp::max as choose_max;

            fn trait_call(
                left: (Timestamp, EventId),
                right: (Timestamp, EventId),
            ) -> std::cmp::Ordering {
                Ord::cmp(&left, &right)
            }
            fn free_call(
                left: (Timestamp, EventId),
                right: (Timestamp, EventId),
            ) -> (Timestamp, EventId) {
                std::cmp::min(left, right)
            }
            fn imported_alias_call(
                left: (Timestamp, EventId),
                right: (Timestamp, EventId),
            ) -> (Timestamp, EventId) {
                choose_max(left, right)
            }
        ",
    )]);
    for name in ["trait_call", "free_call", "imported_alias_call"] {
        let function = corpus
            .functions
            .iter()
            .find(|function| function.id.name == name)
            .expect("call-form ordering fixture");
        assert!(
            !direct(function, &corpus).raw_ordering.is_empty(),
            "call-form {name} ordering must be rejected"
        );
    }
}

#[test]
fn unrelated_branch_effect_is_not_an_owner_controlled_sink() {
    let corpus = Corpus::parse_fixture(&[(
        "crates/example/src/lib.rs",
        r"
            fn checked(
                left: (Timestamp, EventId),
                right: (Timestamp, EventId),
            ) -> bool {
                let mut telemetry = false;
                if event_is_newer(left, right) {
                    telemetry = true;
                }
                telemetry
            }
        ",
    )]);
    let checked = corpus
        .functions
        .iter()
        .find(|function| function.id.name == "checked")
        .expect("unrelated branch-effect fixture");
    let use_ = direct(checked, &corpus);
    assert_eq!(use_.live_owner_calls, 1);
    assert_eq!(
        use_.controlled_owner_sinks.len(),
        0,
        "an arbitrary assignment is not exact insertion or refusal proof"
    );
}

#[test]
fn helper_resolution_rejects_module_import_and_signature_decoys() {
    let corpus = Corpus::parse_fixture(&[(
        "crates/example/src/lib.rs",
        r"
            mod approved {
                fn helper(
                    left: (Timestamp, EventId),
                    right: (Timestamp, EventId),
                ) -> bool {
                    event_is_newer(left, right)
                }
                fn helper(
                    left: Timestamp,
                    right: Timestamp,
                ) -> bool {
                    left > right
                }
            }
            mod hostile {
                fn helper(
                    left: (Timestamp, EventId),
                    right: (Timestamp, EventId),
                ) -> bool {
                    left > right
                }
            }
            use approved::helper as selected_helper;
            fn sink(
                left: (Timestamp, EventId),
                right: (Timestamp, EventId),
            ) -> bool {
                selected_helper(left, right)
            }
        ",
    )]);
    let sink = corpus
        .functions
        .iter()
        .find(|function| function.id.module.is_empty() && function.id.name == "sink")
        .expect("import-resolved sink fixture");
    let (use_, reached) = reachable(sink, &corpus);
    assert_eq!(
        use_.live_owner_calls, 1,
        "the selected helper owns ordering"
    );
    assert!(
        use_.raw_ordering.is_empty(),
        "hostile sibling-module and wrong-signature helpers are unreachable"
    );
    assert_eq!(
        reached
            .iter()
            .filter(|id| id.name == "helper")
            .map(|id| (id.module.as_str(), id.signature.as_str()))
            .collect::<Vec<_>>(),
        vec![(
            "approved",
            "fnhelper(left:(Timestamp,EventId),right:(Timestamp,EventId),)->bool"
        )],
        "the imported module and called signature identify one helper"
    );
}
