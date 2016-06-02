//! A lint that collects metadata
#![feature(plugin_registrar, rustc_private)]

#[macro_use]
extern crate rustc;
extern crate rustc_plugin;
extern crate syntax;

use std::fs::File;
use std::io::{BufWriter, Write};


use syntax::ast::{Name, NodeId};
use syntax::codemap::Span;
use rustc_plugin::Registry;
use rustc::lint::*;
use rustc::hir::*;
use rustc::hir::intravisit::{FnKind, Visitor, walk_expr};

#[plugin_registrar]
pub fn plugin_registrar(reg: &mut Registry) {
    reg.register_late_lint_pass(Box::new(Metacollect::new()));
}

declare_lint! {
    pub METACOLLECT,
    Allow,
    "collect metadata"
}

pub struct Metacollect {
    types_file: BufWriter<File>,
    fn_file: BufWriter<File>,
    current_crate: String,
    itemstack: Vec<Name>,
}

impl Metacollect {
    pub fn new() -> Metacollect {
        let types_file = File::create("target/nsa_types.txt").unwrap(); //TODO
        let fn_file = File::create("target/nsa_funcs.txt").unwrap(); //TODO
        Metacollect {
            types_file: BufWriter::new(types_file),
            fn_file: BufWriter::new(fn_file),
            current_crate: "".into(),
            itemstack: Vec::new(),
        }
    }
}

impl LintPass for Metacollect {
    fn get_lints(&self) -> LintArray {
        lint_array!(METACOLLECT)
    }
}

impl LateLintPass for Metacollect {
    fn check_crate(&mut self, cx: &LateContext, _: &Crate) {
        self.current_crate = (&cx.tcx.crate_name).to_string()
    }
    
    fn check_crate_post(&mut self, _: &LateContext, _: &Crate) {
        if !self.itemstack.is_empty() {
            self.itemstack.clear();
            bug!("itemstack not empty on leaving crate");
        }
        self.current_crate = "".into();
        let _ = self.types_file.flush();
    }

    fn check_item(&mut self, _: &LateContext, item: &Item) {
        self.itemstack.push(item.name);
        match item.node {
            ItemEnum(ref def, ref generics) =>
                for variant in &def.variants {
                    insert_variant_data(self, &variant.node.data, generics)
                },
            ItemStruct(ref data, ref generics) =>
                insert_variant_data(self, data, generics),
            _ => ()
        }
    }

    fn check_item_post(&mut self, _: &LateContext, _: &Item) {
        let _ = self.itemstack.pop();
    }
    
    fn check_fn(&mut self, cx: &LateContext, k: FnKind, _: &FnDecl, 
                block: &Block, _: Span, _: NodeId) {
        match k {
            FnKind::ItemFn(name, _, _, _, _, _, _) | 
            FnKind::Method(name, _, _, _) => insert_fn(self, cx, name, block),
            _ => ()
        }
    }
}

fn insert_fn(nsa: &mut Metacollect, cx: &LateContext, name: Name, block: &Block) {
    let mut visitor = FnVisitor::new(cx, nsa, name);
    visitor.visit_block(block);
}

fn insert_variant_data(meta: &mut Metacollect, def: &VariantData, generics: &Generics) {
    match *def {
        VariantData::Struct(ref subtypes, _) |
        VariantData::Tuple(ref subtypes, _) => 
            insert_struct_fields(meta, subtypes, generics),
        _ => ()
    }
}

fn insert_struct_fields(meta: &mut Metacollect, subtypes: &[StructField], _generics: &Generics) {
    for subty in subtypes {
        //TODO: handle generics
        //TODO: Check output of Ty_
        {
            let (ref mut file, ref current_crate, ref items) = 
                (&mut meta.types_file, &meta.current_crate, &meta.itemstack);
            insert_item_path(file, current_crate, items);
        }
        let _ = writeln!(meta.types_file, "\t{:?}", subty.ty.node);
    }
}

fn insert_call(meta: &mut Metacollect, name: Name, path: &Path) {
    {
        let (ref mut file, ref current_crate, ref items) = 
            (&mut meta.fn_file, &meta.current_crate, &meta.itemstack);
        insert_item_path(file, current_crate, items);
    }
    let _ = writeln!(meta.fn_file, "{}\t{}", name.as_str(), path);
}

fn insert_item_path(file: &mut BufWriter<File>, krate: &String, items: &[Name]) {
    let _ = write!(file, "{}", krate);
    for name in items {
        let _ = write!(file, "::{}", &*name.as_str());
    }
}

struct FnVisitor<'a, 'c, 't: 'c> {
    cx: &'c LateContext<'c, 't>,
    meta: &'a mut Metacollect,
    name: Name
}

impl<'a, 'c, 't: 'c> FnVisitor<'a, 'c, 't> {
    fn new(cx: &'c LateContext<'c, 't>, meta: &'a mut Metacollect, name: Name) -> Self {
        FnVisitor { cx: cx, meta: meta, name: name }
    }
}

static DEREF : &'static [&'static str] = &["std", "ops", "Deref"];
static NOT   : &'static [&'static str] = &["std", "ops", "Not"];
static NEG   : &'static [&'static str] = &["std", "ops", "Neg"];

impl<'a, 'c, 't: 'c> Visitor<'c> for FnVisitor<'a, 'c, 't> {
    /// look up method calls and some ops (which are implemented by traits)
    fn visit_expr(&mut self, expr: &'c Expr) {
        match expr.node {
            ExprMethodCall(ref _target_name, ref _tys, ref _args) => {
                // TODO: target_name is the name of the function that we need
                // to look up in the 0th type
                // this will use self.cx
            },
            ExprCall(ref function, ref _args) =>
                // target is a Spanned{ ExprPath } here
                if let ExprPath(ref _qself, ref path) = function.node {
                    //TODO: look up path, what to do with qself?
                    insert_call(self.meta, self.name, path);
                },
            ExprUnary(op, ref arg) => {
                let trait_path : &'static [&str] = match op {
                    UnDeref => DEREF, //TODO DerefMut?
                    UnNot => NOT,
                    UnNeg => NEG
                };
                insert_unop(self, trait_path, arg)
            },
            ExprBinary(op, ref l, ref r) => {
                match op.node {
                    BiAdd => insert_binop(self, &["core", "ops", "Add"], l, r),
                    BiSub => insert_binop(self, &["core", "ops", "Sub"], l, r),
                    BiMul => insert_binop(self, &["core", "ops", "Mul"], l, r),
                    BiDiv => insert_binop(self, &["core", "ops", "Div"], l, r),
                    BiRem => insert_binop(self, &["core", "ops", "Rem"], l, r),
                    BiBitXor => insert_binop(self, &["core", "ops", "BitXor"], l, r),
                    BiBitAnd => insert_binop(self, &["core", "ops", "BitAnd"], l, r),
                    BiBitOr => insert_binop(self, &["core", "ops", "BitOr"], l, r),
                    BiShl => insert_binop(self, &["core", "ops", "Shl"], l, r),
                    BiShr => insert_binop(self, &["core", "ops", "Shr"], l, r),
                    BiEq |
                    BiNe => insert_binop(self, &["core", "cmp", "PartialEq"], l, r),
                    BiLt |
                    BiLe |
                    BiGe |
                    BiGt => insert_cmp(self, l, r),
                    BiAnd |
                    BiOr => (),
                }
            },
            ExprAssignOp(op, ref l, ref r) => {
                match op.node {
                    BiAdd => insert_op_assign(self, "AddAssign", "Add", l, r),
                    BiSub => insert_op_assign(self, "SubAssign", "Sub", l, r),
                    BiMul => insert_op_assign(self, "MulAssign", "Mul", l, r),
                    BiDiv => insert_op_assign(self, "DivAssign", "Div", l, r),
                    BiRem => insert_op_assign(self, "RemAssign", "Rem", l, r),
                    BiBitXor => insert_op_assign(self, "BitXorAssign", "BitXor", l, r),
                    BiBitAnd => insert_op_assign(self, "BitAndAssign", "BitAnd", l, r),
                    BiBitOr => insert_op_assign(self, "BitOrAssign", "BitOr", l, r),
                    BiShl => insert_op_assign(self, "ShlAssign", "Shl", l, r),
                    BiShr => insert_op_assign(self, "ShrAssign", "Shr", l, r),
                    _ => (),
                }
            },
            ExprIndex(ref l, ref r) =>
                insert_binop(self, &["core", "ops", "Index"], l, r), //TODO: IndexMut?
            _ => (),
        }
        walk_expr(self, expr);
    }
}

fn insert_unop(y: &mut FnVisitor, trait_path: &[&str], arg: &Expr) {
    //TODO
}

fn insert_binop(v: &mut FnVisitor, trait_path: &[&str], l: &Expr, r: &Expr) {
    if let Some(ref node) = lookup_bi_trait(v, trait_path, l, r) {
        //TODO
    }
}

fn insert_cmp(v: &mut FnVisitor, l: &Expr, r: &Expr) {
    if let Some(ref node) = lookup_bi_trait(v, &["core", "cmp", "Ord"], l, r)
                .or_else(|| lookup_bi_trait(v, &["core", "cmp", "PartialOrd"], l, r)) {
        //TODO
    }
}

fn insert_op_assign(v: &mut FnVisitor, op_assign: &str, op: &str, l: &Expr, r: &Expr) {
    if let Some(ref node) = lookup_bi_trait(v, &["core", "ops", op_assign], l, r)
                .or_else(|| lookup_bi_trait(v, &["core", "ops", op], l, r)) {
        //TODO
    }
}

fn lookup_bi_trait(v: &mut FnVisitor, path: &[&str], l: &Expr, r: &Expr)
-> Option<NodeId> {
    None //TODO
}
