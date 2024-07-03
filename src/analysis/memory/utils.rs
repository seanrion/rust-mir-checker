// This file is adapted from MIRAI (https://github.com/facebookexperimental/MIRAI)
// Original author: Herman Venter <hermanv@fb.com>
// Original copyright header:

// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use log::debug;
use rustc_hir::def_id::DefId;
use rustc_hir::definitions::DefPathData;
use rustc_middle::ty;
// use rustc_middle::ty::print::{FmtPrinter, Printer};
use rustc_middle::ty::subst::{GenericArgKind, SubstsRef};
use rustc_middle::ty::{DefIdTree, ProjectionTy, Ty, TyCtxt, TyKind};
use std::rc::Rc;

/// Appends a string to str with the constraint that it must uniquely identify ty and also
/// be a valid identifier (so that core library contracts can be written for type specialized
/// generic trait methods).
fn append_mangled_type<'tcx>(str: &mut String, ty: Ty<'tcx>, tcx: TyCtxt<'tcx>) {
    trace!("append_mangled_type {:?} to {}", ty.kind(), str);
    use TyKind::*;
    match ty.kind() {
        Bool => str.push_str("bool"),
        Char => str.push_str("char"),
        Int(int_ty) => {
            str.push_str(match int_ty {
                rustc_middle::ty::IntTy::Isize => "isize",
                rustc_middle::ty::IntTy::I8 => "i8",
                rustc_middle::ty::IntTy::I16 => "i16",
                rustc_middle::ty::IntTy::I32 => "i32",
                rustc_middle::ty::IntTy::I64 => "i64",
                rustc_middle::ty::IntTy::I128 => "i128",
            });
        }
        Uint(uint_ty) => {
            str.push_str(match uint_ty {
                rustc_middle::ty::UintTy::Usize => "usize",
                rustc_middle::ty::UintTy::U8 => "u8",
                rustc_middle::ty::UintTy::U16 => "u16",
                rustc_middle::ty::UintTy::U32 => "u32",
                rustc_middle::ty::UintTy::U64 => "u64",
                rustc_middle::ty::UintTy::U128 => "u128",
            });
        }
        Float(float_ty) => {
            str.push_str(match float_ty {
                rustc_middle::ty::FloatTy::F32 => "f32",
                rustc_middle::ty::FloatTy::F64 => "f64",
            });
        }
        Adt(def, subs) => {
            str.push_str(qualified_type_name(tcx, def.did).as_str());
            for sub in subs.into_iter() {
                if let GenericArgKind::Type(ty) = sub.unpack() {
                    str.push('_');
                    append_mangled_type(str, ty, tcx);
                }
            }
        }
        Closure(def_id, subs) => {
            str.push_str("closure_");
            str.push_str(qualified_type_name(tcx, *def_id).as_str());
            for sub in subs.as_closure().substs {
                if let GenericArgKind::Type(ty) = sub.unpack() {
                    str.push('_');
                    append_mangled_type(str, ty, tcx);
                }
            }
        }
        Dynamic(trait_data, ..) => {
            str.push_str("trait_");
            if let Some(principal) = trait_data.principal() {
                let principal =
                    tcx.normalize_erasing_late_bound_regions(ty::ParamEnv::reveal_all(), principal);
                str.push_str(qualified_type_name(tcx, principal.def_id).as_str());
                for sub in principal.substs {
                    if let GenericArgKind::Type(ty) = sub.unpack() {
                        str.push('_');
                        append_mangled_type(str, ty, tcx);
                    }
                }
            }
        }
        Foreign(def_id) => {
            str.push_str("extern_type_");
            str.push_str(qualified_type_name(tcx, *def_id).as_str());
        }
        FnDef(def_id, subs) => {
            str.push_str("fn_");
            str.push_str(qualified_type_name(tcx, *def_id).as_str());
            for sub in subs.into_iter() {
                if let GenericArgKind::Type(ty) = sub.unpack() {
                    str.push('_');
                    append_mangled_type(str, ty, tcx);
                }
            }
        }
        Generator(def_id, subs, ..) => {
            str.push_str("generator_");
            str.push_str(qualified_type_name(tcx, *def_id).as_str());
            for sub in subs.as_generator().substs {
                if let GenericArgKind::Type(ty) = sub.unpack() {
                    str.push('_');
                    append_mangled_type(str, ty, tcx);
                }
            }
        }
        GeneratorWitness(binder) => {
            for ty in binder.skip_binder().iter() {
                str.push('_');
                append_mangled_type(str, ty, tcx)
            }
        }
        Opaque(def_id, subs) => {
            str.push_str("impl_");
            str.push_str(qualified_type_name(tcx, *def_id).as_str());
            for sub in subs.into_iter() {
                if let GenericArgKind::Type(ty) = sub.unpack() {
                    str.push('_');
                    append_mangled_type(str, ty, tcx);
                }
            }
        }
        Str => str.push_str("str"),
        Array(ty, _) => {
            str.push_str("array_");
            append_mangled_type(str, ty, tcx);
        }
        Slice(ty) => {
            str.push_str("slice_");
            append_mangled_type(str, ty, tcx);
        }
        RawPtr(ty_and_mut) => {
            str.push_str("pointer_");
            match ty_and_mut.mutbl {
                rustc_hir::Mutability::Mut => str.push_str("mut_"),
                rustc_hir::Mutability::Not => str.push_str("const_"),
            }
            append_mangled_type(str, ty_and_mut.ty, tcx);
        }
        Ref(_, ty, mutability) => {
            str.push_str("ref_");
            if *mutability == rustc_hir::Mutability::Mut {
                str.push_str("mut_");
            }
            append_mangled_type(str, ty, tcx);
        }
        FnPtr(poly_fn_sig) => {
            let fn_sig = poly_fn_sig.skip_binder();
            str.push_str("fn_ptr_");
            for arg_type in fn_sig.inputs() {
                append_mangled_type(str, arg_type, tcx);
                str.push('_');
            }
            append_mangled_type(str, fn_sig.output(), tcx);
        }
        Tuple(types) => {
            str.push_str("tuple_");
            str.push_str(&format!("{}", types.len()));
            types.iter().for_each(|t| {
                str.push('_');
                append_mangled_type(str, t.expect_ty(), tcx);
            });
        }
        Param(param_ty) => {
            str.push_str("generic_par_");
            str.push_str(&param_ty.name.as_str());
        }
        Projection(projection_ty) => {
            append_mangled_type(str, projection_ty.self_ty(), tcx);
            str.push_str("_as_");
            str.push_str(qualified_type_name(tcx, projection_ty.item_def_id).as_str());
        }
        Never => {
            str.push('_');
        }
        _ => {
            //todo: add cases as the need arises, meanwhile make the need obvious.
            debug!("{:?}", ty);
            debug!("{:?}", ty.kind());
            str.push_str(&format!("default formatted {:?}", ty))
        }
    }
}

/// Pretty much the same as summary_key_str but with _ used rather than . so that
/// the result can be appended to a valid identifier.
fn qualified_type_name(tcx: TyCtxt<'_>, def_id: DefId) -> String {
    let mut name = crate_name(tcx, def_id);
    for component in &tcx.def_path(def_id).data {
        name.push('_');
        push_component_name(component.data, &mut name);
        if component.disambiguator != 0 {
            name.push('_');
            let da = component.disambiguator.to_string();
            name.push_str(da.as_str());
        }
    }
    name
}

/// Constructs a name for the crate that contains the given def_id.
fn crate_name(tcx: TyCtxt<'_>, def_id: DefId) -> String {
    tcx.crate_name(def_id.krate).as_str().to_string()
}

/// Constructs a string that uniquely identifies a definition to serve as a key to
/// the summary cache, which is a key value store. The string will always be the same as
/// long as the definition does not change its name or location, so it can be used to
/// transfer information from one compilation to the next, making incremental analysis possible.
pub fn summary_key_str(tcx: TyCtxt<'_>, def_id: DefId) -> Rc<String> {
    let mut name = crate_name(tcx, def_id);
    let mut type_ns: Option<String> = None;
    for component in &tcx.def_path(def_id).data {
        if name.ends_with("foreign_contracts") {
            // By stripping off this special prefix, we allow this crate (or module) to define
            // functions that appear to be from other crates.
            // We use this to provide contracts for functions defined in crates we do not
            // wish to modify in place.
            name.clear();
        } else if !name.ends_with('.') {
            name.push('.');
        }
        push_component_name(component.data, &mut name);
        if let DefPathData::TypeNs(sym) = component.data {
            type_ns = Some(sym.as_str().to_string());
        }
        if component.disambiguator != 0 {
            name.push('_');
            if component.data == DefPathData::Impl {
                if let Some(parent_def_id) = tcx.parent(def_id) {
                    if let Some(type_ns) = &type_ns {
                        if type_ns == "num"
                            && tcx.crate_name(parent_def_id.krate).as_str() == "core"
                        {
                            append_mangled_type(&mut name, tcx.type_of(parent_def_id), tcx);
                            continue;
                        }
                    }
                }
                if let Some(type_ns) = &type_ns {
                    name.push_str(&type_ns);
                    continue;
                }
            }
            let da = component.disambiguator.to_string();
            name.push_str(da.as_str());
        }
    }
    Rc::new(name)
}

fn push_component_name(component_data: DefPathData, target: &mut String) {
    use std::ops::Deref;
    use DefPathData::*;
    match component_data {
        TypeNs(name) | ValueNs(name) | MacroNs(name) | LifetimeNs(name) => {
            target.push_str(name.as_str().deref());
        }
        _ => target.push_str(match component_data {
            CrateRoot => "crate_root",
            Impl => "implement",
            Misc => "miscellaneous",
            ClosureExpr => "closure",
            Ctor => "ctor",
            AnonConst => "constant",
            ImplTrait => "implement_trait",
            _ => unreachable!(),
        }),
    };
}

/// Returns false if any of the generic arguments are themselves generic
pub fn are_concrete(gen_args: SubstsRef<'_>) -> bool {
    for gen_arg in gen_args.iter() {
        if let GenericArgKind::Type(ty) = gen_arg.unpack() {
            if !is_concrete(&ty.kind()) {
                return false;
            }
        }
    }
    true
}

/// Determines if the given type is fully concrete.
pub fn is_concrete(ty: &TyKind<'_>) -> bool {
    match ty {
        TyKind::Bound(..) | TyKind::Param(..) | TyKind::Infer(..) | TyKind::Error(..) => false,
        TyKind::Adt(_, gen_args)
        | TyKind::Closure(_, gen_args)
        | TyKind::FnDef(_, gen_args)
        | TyKind::Generator(_, gen_args, _)
        | TyKind::Opaque(_, gen_args)
        | TyKind::Projection(ProjectionTy {
            substs: gen_args, ..
        })
        | TyKind::Tuple(gen_args) => are_concrete(gen_args),
        TyKind::Ref(_, ty, _) => is_concrete(&ty.kind()),
        _ => true,
    }
}
