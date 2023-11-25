use matches2::option_match;
use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::Error;

use super::opt;
use crate::util::{Attr, Result};

pub(super) enum ArgType {
    Local {
        default:       Option<Box<syn::Expr>>,
        referrer_attr: Option<bool>,
    },
    Global {
        thread_local: bool,
        maybe_uninit: Vec<syn::Type>,
    },
    Simple {
        mutable:      bool,
        arch:         Box<syn::Type>,
        comp:         Box<syn::Type>,
        maybe_uninit: Vec<syn::Type>,
    },
    Isotope {
        mutable:      bool,
        arch:         Box<syn::Type>,
        comp:         Box<syn::Type>,
        discrim:      Option<Box<syn::Expr>>,
        discrim_set:  Result<Box<syn::Type>, Span>,
        maybe_uninit: Vec<syn::Type>,
    },
    EntityCreator {
        arch:         Box<syn::Type>,
        no_partition: bool,
    },
    EntityDeleter {
        arch: Box<syn::Type>,
    },
    EntityIterator {
        arch: Box<syn::Type>,
    },
}
type PartialArgTypeBuilder = Box<dyn FnOnce(&str, &[&syn::Type], Span) -> Result<ArgType>>;

enum MaybePartial {
    Full(ArgType),
    Partial(PartialArgTypeBuilder),
}

fn simple_partial_builder(mutable: bool, maybe_uninit: Vec<syn::Type>) -> PartialArgTypeBuilder {
    Box::new(move |ident, args, args_span| {
        let [arch, comp]: [&syn::Type; 2] = args.try_into().map_err(|_| {
            Error::new(
                args_span,
                "Cannot infer archetype and component for component access. Specify explicitly \
                 with `#[dynec(simple(arch = X, comp = Y))]`, or use `ReadSimple<X, \
                 Y>`/`WriteSimple<X, Y>`.",
            )
        })?;

        Ok(ArgType::Simple {
            mutable: mutable || ident == "WriteSimple",
            arch: Box::new(arch.clone()),
            comp: Box::new(comp.clone()),
            maybe_uninit,
        })
    })
}

fn isotope_partial_builder(
    mutable: bool,
    discrim: Option<Box<syn::Expr>>,
    maybe_uninit: Vec<syn::Type>,
) -> PartialArgTypeBuilder {
    Box::new(move |ident, args, args_span| {
        if args.len() != 2 && args.len() != 3 {
            return Err(Error::new(
                args_span,
                "Cannot infer archetype and component for component access. Specify explicitly \
                 with `#[dynec(isotope(arch = X, comp = Y, [discrim_set = Z]))]`, or use \
                 `(Read|Write)Isotope(Full|Isotope)<X, Y, [Z]>`.",
            ));
        }

        let &arch = args.first().expect("args.len() >= 2");
        let &comp = args.get(1).expect("args.len() >= 2");
        let discrim_set = args.get(2).map(|&ty| Box::new(ty.clone())).ok_or(args_span);

        Ok(ArgType::Isotope {
            mutable: mutable || ident.starts_with("WriteIsotope"),
            arch: Box::new(arch.clone()),
            comp: Box::new(comp.clone()),
            discrim,
            discrim_set,
            maybe_uninit,
        })
    })
}

fn entity_creator_partial_builder(no_partition: bool) -> PartialArgTypeBuilder {
    Box::new(move |_, args, args_span| {
        let [arch]: [&syn::Type; 1] = args.try_into().map_err(|_| {
            Error::new(
                args_span,
                "Cannot infer archetype for entity creation. Specify explicitly with \
                 `#[dynec(entity_creator(arch = X))]`, or use `impl EntityCreator<X>`.",
            )
        })?;

        Ok(ArgType::EntityCreator { arch: Box::new(arch.clone()), no_partition })
    })
}

fn entity_deleter_partial_builder() -> PartialArgTypeBuilder {
    Box::new(move |_, args, args_span| {
        let [arch]: [&syn::Type; 1] = args.try_into().map_err(|_| {
            Error::new(
                args_span,
                "Cannot infer archetype for entity deletion. Specify explicitly with \
                 `#[dynec(entity_deleter(arch = X))]`, or use `impl EntityDeleter<X>`.",
            )
        })?;

        Ok(ArgType::EntityDeleter { arch: Box::new(arch.clone()) })
    })
}

fn entity_iterator_partial_builder() -> PartialArgTypeBuilder {
    Box::new(move |_, args, args_span| {
        let [arch]: [&syn::Type; 1] = args.try_into().map_err(|_| {
            Error::new(
                args_span,
                "Cannot infer archetype for entity iteration. Specify explicitly with \
                 `#[dynec(entity_iterator(arch = X))]`, or use `impl EntityIterator<X>`.",
            )
        })?;

        Ok(ArgType::EntityIterator { arch: Box::new(arch.clone()) })
    })
}

const USAGE_INFERENCE_ERROR: &str =
    "Cannot infer parameter usage. Specify explicitly with `#[dynec(...)]`, or use the form `impl \
     system::(Read|Write)(Simple|Isotope)<Arch, Comp>` or `impl system::Entity(Creator|Deleter)`.";

pub(super) fn infer_arg_type(param: &mut syn::PatType) -> Result<ArgType> {
    let mut maybe_partial: Option<MaybePartial> = None;

    let param_span = param.span();
    for attr in param.attrs.extract_if(|attr| attr.path().is_ident("dynec")) {
        let arg_attr = attr.parse_args::<Attr<opt::Arg>>()?;

        for arg in arg_attr.items {
            let old =
                maybe_partial.replace(try_attr_to_arg_type(arg.value, attr.span(), param_span)?);
            if old.is_some() {
                return Err(Error::new(
                    param_span,
                    "Each argument can only have one argument type",
                ));
            }
        }
    }

    let arg_type = match maybe_partial {
        Some(MaybePartial::Full(arg_type)) => arg_type,
        arg_type => {
            let syn::Type::Path(ty) = &*param.ty else {
                return Err(Error::new_spanned(&*param.ty, USAGE_INFERENCE_ERROR));
            };

            let trait_name = ty.path.segments.last().expect("path should not be empty");
            let trait_name_string = trait_name.ident.to_string();

            let builder = match arg_type {
                Some(MaybePartial::Partial(builder)) => builder,
                None => match trait_name_string.as_str() {
                    "ReadSimple" | "WriteSimple" => simple_partial_builder(false, Vec::new()),
                    "ReadIsotopePartial"
                    | "WriteIsotopePartial"
                    | "ReadIsotopeFull"
                    | "WriteIsotopeFull" => isotope_partial_builder(false, None, Vec::new()),
                    "EntityCreator" => entity_creator_partial_builder(false),
                    "EntityDeleter" => entity_deleter_partial_builder(),
                    "EntityIterator" => entity_iterator_partial_builder(),
                    _ => return Err(Error::new_spanned(trait_name, USAGE_INFERENCE_ERROR)),
                },
                _ => unreachable!(),
            };

            let type_args = match &trait_name.arguments {
                syn::PathArguments::AngleBracketed(args) => args,
                _ => return Err(Error::new_spanned(&trait_name.arguments, USAGE_INFERENCE_ERROR)),
            };
            let types: Vec<&syn::Type> = type_args
                .args
                .iter()
                .map(|arg| match arg {
                    syn::GenericArgument::Type(ty) => Ok(ty),
                    _ => Err(Error::new_spanned(arg, USAGE_INFERENCE_ERROR)),
                })
                .collect::<Result<_>>()?;
            builder(&trait_name_string, &types, type_args.span())?
        }
    };
    Ok(arg_type)
}

fn try_attr_to_arg_type(arg: opt::Arg, attr_span: Span, param_span: Span) -> Result<MaybePartial> {
    let maybe = match arg {
        opt::Arg::Param(_, opts) => {
            let referrer_attr = opts
                .find_one(|opt| match opt {
                    opt::ParamArg::HasEntity => Some(&true),
                    opt::ParamArg::HasNoEntity => Some(&false),
                })?
                .map(|(_, &bool)| bool);

            MaybePartial::Full(ArgType::Local { default: None, referrer_attr })
        }
        opt::Arg::Local(_, opts) => {
            let initial = match opts
                .find_one(|opt| option_match!(opt, opt::LocalArg::Initial(_, initial) => initial))?
            {
                Some((_, initial)) => initial,
                None => {
                    return Err(Error::new(
                        attr_span,
                        "Missing required expression for #[dynec(local(initial = expr))]",
                    ))
                }
            };

            let referrer_attr = opts
                .find_one(|opt| match opt {
                    opt::LocalArg::HasEntity => Some(&true),
                    opt::LocalArg::HasNoEntity => Some(&false),
                    _ => None,
                })?
                .map(|(_, &bool)| bool);

            MaybePartial::Full(ArgType::Local { default: Some(initial.clone()), referrer_attr })
        }
        opt::Arg::Global(_, opts) => {
            let thread_local = opts
                .find_one(|opt| option_match!(opt, opt::GlobalArg::ThreadLocal => &()))?
                .is_some();
            let maybe_uninit = opts.merge_all(|opt| option_match!(opt, opt::GlobalArg::MaybeUninit(_, tys) => tys.iter().cloned()));
            MaybePartial::Full(ArgType::Global { thread_local, maybe_uninit })
        }
        opt::Arg::Simple(_, opts) => {
            let mutable =
                opts.find_one(|opt| option_match!(opt, opt::SimpleArg::Mutable => &()))?.is_some();
            let arch =
                opts.find_one(|opt| option_match!(opt, opt::SimpleArg::Arch(_, ty) => ty))?;
            let comp =
                opts.find_one(|opt| option_match!(opt, opt::SimpleArg::Comp(_, ty) => ty))?;
            let maybe_uninit = opts.merge_all(|opt| option_match!(opt, opt::SimpleArg::MaybeUninit(_, tys) => tys.iter().cloned()));

            match (arch, comp, mutable) {
                (Some((_, arch)), Some((_, comp)), mutable) => {
                    MaybePartial::Full(ArgType::Simple {
                        mutable,
                        arch: arch.clone(),
                        comp: comp.clone(),
                        maybe_uninit,
                    })
                }
                (None, None, false) => {
                    MaybePartial::Partial(simple_partial_builder(mutable, maybe_uninit))
                }
                _ => {
                    return Err(Error::new(
                        attr_span,
                        "Invalid argument. `arch`, `comp` and `mutable` have no effect unless \
                         both `arch` and `comp` are supplied.",
                    ));
                }
            }
        }
        opt::Arg::Isotope(_, opts) => {
            let mutable =
                opts.find_one(|opt| option_match!(opt, opt::IsotopeArg::Mutable => &()))?.is_some();
            let arch =
                opts.find_one(|opt| option_match!(opt, opt::IsotopeArg::Arch(_, ty) => ty))?;
            let comp =
                opts.find_one(|opt| option_match!(opt, opt::IsotopeArg::Comp(_, ty) => ty))?;
            let discrim = opts.find_one(
                |opt| option_match!(opt, opt::IsotopeArg::Discrim(_, discrim) => discrim),
            )?;
            let discrim_set = opts
                .find_one(|opt| option_match!(opt, opt::IsotopeArg::DiscrimSet(_, ty) => ty))?
                .ok_or(param_span);
            let maybe_uninit = opts.merge_all(|opt| option_match!(opt, opt::IsotopeArg::MaybeUninit(_, tys) => tys.iter().cloned()));

            match (arch, comp, mutable) {
                (Some((_, arch)), Some((_, comp)), mutable) => {
                    MaybePartial::Full(ArgType::Isotope {
                        mutable,
                        arch: arch.clone(),
                        comp: comp.clone(),
                        discrim: discrim.map(|(_, discrim)| discrim.clone()),
                        discrim_set: discrim_set.map(|(_, ty)| ty.clone()),
                        maybe_uninit,
                    })
                }
                (None, None, false) => MaybePartial::Partial(isotope_partial_builder(
                    mutable,
                    discrim.map(|(_, expr)| expr.clone()),
                    maybe_uninit,
                )),
                _ => {
                    return Err(Error::new(
                        attr_span,
                        "Invalid argument. `arch`, `comp` and `mutable` have no effect unless \
                         both `arch` and `comp` are supplied.",
                    ));
                }
            }
        }
        opt::Arg::EntityCreator(_, opts) => {
            let arch =
                opts.find_one(|opt| option_match!(opt, opt::EntityCreatorArg::Arch(_, ty) => ty))?;
            let no_partition = opts
                .find_one(|opt| option_match!(opt, opt::EntityCreatorArg::NoPartition => &()))?
                .is_some();

            match arch {
                Some((_, arch)) => {
                    MaybePartial::Full(ArgType::EntityCreator { arch: arch.clone(), no_partition })
                }
                None => MaybePartial::Partial(entity_creator_partial_builder(no_partition)),
            }
        }
        opt::Arg::EntityDeleter(_, opts) => {
            let arch =
                opts.find_one(|opt| option_match!(opt, opt::EntityDeleterArg::Arch(_, ty) => ty))?;

            match arch {
                Some((_, arch)) => {
                    MaybePartial::Full(ArgType::EntityDeleter { arch: arch.clone() })
                }
                None => MaybePartial::Partial(entity_deleter_partial_builder()),
            }
        }
        opt::Arg::EntityIterator(_, opts) => {
            let arch =
                opts.find_one(|opt| option_match!(opt, opt::EntityIteratorArg::Arch(_, ty) => ty))?;

            match arch {
                Some((_, arch)) => {
                    MaybePartial::Full(ArgType::EntityIterator { arch: arch.clone() })
                }
                None => MaybePartial::Partial(entity_iterator_partial_builder()),
            }
        }
    };
    Ok(maybe)
}
