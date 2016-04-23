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
    reg.register_late_lint_pass(Box::new(Nsa::new()));
}

declare_lint! {
    pub NSA,
    Allow,
    "collect metadata"
}

pub struct Nsa {
    types_file: BufWriter<File>,
    fn_file: BufWriter<File>,
    current_crate: String,
    itemstack: Vec<Name>,
}

impl Nsa {
    pub fn new() -> Nsa {
        let types_file = File::create("target/nsa_types.txt").unwrap(); //TODO
        let fn_file = File::create("target/nsa_funcs.txt").unwrap(); //TODO
        Nsa {
            types_file: BufWriter::new(types_file),
            fn_file: BufWriter::new(fn_file),
            current_crate: "".into(),
            itemstack: Vec::new(),
        }
    }
}

impl LintPass for Nsa {
    fn get_lints(&self) -> LintArray {
        lint_array!(NSA)
    }
}

impl LateLintPass for Nsa {
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

fn insert_fn(nsa: &mut Nsa, cx: &LateContext, name: Name, block: &Block) {
    let mut visitor = FnVisitor::new(cx, nsa, name);
    visitor.visit_block(block);
}

fn insert_variant_data(nsa: &mut Nsa, def: &VariantData, generics: &Generics) {
    match *def {
        VariantData::Struct(ref subtypes, _) |
        VariantData::Tuple(ref subtypes, _) => 
            insert_struct_fields(nsa, subtypes, generics),
        _ => ()
    }
}

fn insert_struct_fields(nsa: &mut Nsa, subtypes: &[StructField], _generics: &Generics) {
    for subty in subtypes {
        //TODO: handle generics
        //TODO: Check output of Ty_
        {
            let (ref mut file, ref current_crate, ref items) = 
                (&mut nsa.types_file, &nsa.current_crate, &nsa.itemstack);
            insert_item_path(file, current_crate, items);
        }
        let _ = writeln!(nsa.types_file, "\t{:?}", subty.ty.node);
    }
}

fn insert_call(nsa: &mut Nsa, name: Name, path: &Path) {
    {
        let (ref mut file, ref current_crate, ref items) = 
            (&mut nsa.fn_file, &nsa.current_crate, &nsa.itemstack);
        insert_item_path(file, current_crate, items);
    }
    let _ = writeln!(nsa.fn_file, "{}\t{}", name.as_str(), path);
}

fn insert_item_path(file: &mut BufWriter<File>, krate: &String, items: &[Name]) {
    let _ = write!(file, "{}", krate);
    for name in items {
        let _ = write!(file, "::{}", &*name.as_str());
    }
}

struct FnVisitor<'a, 'c, 't: 'c> {
    cx: &'c LateContext<'c, 't>,
    nsa: &'a mut Nsa,
    name: Name
}

impl<'a, 'c, 't: 'c> FnVisitor<'a, 'c, 't> {
    fn new(cx: &'c LateContext<'c, 't>, nsa: &'a mut Nsa, name: Name) -> Self {
        FnVisitor { cx: cx, nsa: nsa, name: name }
    }
}

impl<'a, 'c, 't: 'c> Visitor<'c> for FnVisitor<'a, 'c, 't> {
    fn visit_expr(&mut self, expr: &'c Expr) {
        match expr.node {
            ExprMethodCall(ref _target_name, ref _tys, ref _args) => {
                // TODO: target_name is the name of the function that we need
                // to look up in the 0th type
                // this will use self.cx
            },
            ExprCall(ref target, ref _args) =>
                // target is a Spanned{ ExprPath } here
                if let ExprPath(ref _qself, ref path) = target.node {
                    //TODO: look up path, what to do with qself?
                    insert_call(self.nsa, self.name, path);
                },
            _ => (),
        }
        walk_expr(self, expr);
    }
}
