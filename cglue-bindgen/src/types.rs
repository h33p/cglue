use crate::config::Config;
use itertools::Itertools;
use regex::*;
use std::collections::{HashMap, HashSet};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone, Copy)]
pub struct ContainerType<'a> {
    pub ty_prefix: &'a str,
    pub cpp_type: &'a str,
    pub drop_impl: Option<&'a str>,
}

impl<'a> ContainerType<'a> {
    pub fn get_map() -> HashMap<&'a str, Self> {
        [
            (
                "CBox_c_void",
                ContainerType {
                    ty_prefix: "Box",
                    cpp_type: "CBox<void>",
                    drop_impl: Some(
                        "if (self->drop_fn && self->instance) self->drop_fn(self->instance);",
                    ),
                },
            ),
            (
                "____c_void",
                ContainerType {
                    ty_prefix: "Mut",
                    cpp_type: "void *",
                    drop_impl: None,
                },
            ),
            (
                "_____c_void",
                ContainerType {
                    ty_prefix: "Ref",
                    cpp_type: "const void *",
                    drop_impl: None,
                },
            ),
        ]
        .iter()
        .cloned()
        .collect::<HashMap<_, _>>()
    }

    pub fn get_prefix_map() -> HashMap<&'a str, Self> {
        Self::get_map()
            .into_iter()
            .map(|(_, v)| (v.ty_prefix, v))
            .collect()
    }

    pub const fn from_name(name: &'a str) -> Self {
        Self {
            ty_prefix: name,
            cpp_type: name,
            drop_impl: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ContextType<'a> {
    pub ty_prefix: &'a str,
    pub cpp_type: &'a str,
    pub clone_impl: Option<&'a str>,
    pub drop_impl: Option<&'a str>,
}

impl<'a> ContextType<'a> {
    pub fn get_map() -> HashMap<&'a str, Self> {
        [
            (
                "NoContext",
                ContextType {
                    ty_prefix: "",
                    cpp_type: "NoContext",
                    clone_impl: None,
                    drop_impl: None,
                },
            ),
            (
                "CArc_c_void",
                ContextType {
                    ty_prefix: "Arc",
                    cpp_type: "CArc<void>",
                    clone_impl: Some("ret.instance = self->clone_fn(self->instance);"),
                    drop_impl: Some(
                        "if (self->drop_fn && self->instance) self->drop_fn(self->instance);",
                    ),
                },
            ),
        ]
        .iter()
        .cloned()
        .collect::<HashMap<_, _>>()
    }

    pub fn get_prefix_map() -> HashMap<&'a str, Self> {
        Self::get_map()
            .into_iter()
            .map(|(_, v)| (v.ty_prefix, v))
            .collect()
    }

    pub const fn from_name(name: &'a str) -> Self {
        Self {
            ty_prefix: name,
            cpp_type: name,
            drop_impl: None,
            clone_impl: None,
        }
    }
}

#[derive(Clone)]
pub struct FunctionArg {
    ty: String,
    name: String,
}

#[derive(Clone)]
pub struct Function {
    pub name: String,
    return_type: String,
    arguments: Vec<FunctionArg>,
    is_const: bool,
    moves_self: bool,
    calls_vtbl: bool,
}

impl Function {
    pub fn create_wrapper(
        &self,
        (container, vtbl): (&str, &str),
        prefix: &str,
        (cpp_mode, cast_self): (bool, bool),
        (this_ty, vtbls): (&str, &[&str]),
        container_info: (&str, &str, bool),
        context: (&str, &str, bool),
    ) -> String {
        let args = Itertools::intersperse(
            if cpp_mode {
                None
            } else {
                Some(format!(
                    "{}{} {}self",
                    if !self.moves_self && self.is_const {
                        "const "
                    } else {
                        ""
                    },
                    if cast_self { "void" } else { this_ty },
                    if self.moves_self { "" } else { "*" }
                ))
            }
            .into_iter()
            .chain(
                self.arguments
                    .iter()
                    .map(|a| format!("{} {}", a.ty, a.name)),
            ),
            ", ".to_string(),
        )
        .collect::<String>();

        let this_access = if cpp_mode {
            "this->".to_string()
        } else if self.moves_self {
            "self.".to_string()
        } else if !cast_self {
            "self->".to_string()
        } else {
            format!(
                "(({}{} *)self)->",
                if self.is_const { "const " } else { "" },
                this_ty
            )
        };

        let mut call_args = format!(
            "{}{}{}",
            if self.moves_self { "" } else { "&" },
            this_access,
            container
        );

        for a in &self.arguments {
            call_args.push_str(", ");
            call_args.push_str(&a.name);
        }

        let mut copied_vtbls = String::new();

        let return_type = self.return_type.trim();
        let wrap_in_container = return_type == container_info.0;

        if wrap_in_container {
            for v in vtbls {
                copied_vtbls.push_str(&format!("\n        __ret.{} = {}{};", v, this_access, v));
            }
        }

        let mut post_call = if self.moves_self {
            if cpp_mode {
                format!("    mem_forget({}container);\n", this_access)
            } else if context.2 && self.calls_vtbl {
                format!("    ctx_{}_drop(&___ctx);\n", context.1.to_lowercase())
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Manually call into drop implementation if we are not
        // transfering ownership to Rust (only applicable in C)
        if self.moves_self && !self.calls_vtbl && !cpp_mode {
            if container_info.2 {
                post_call += &format!(
                    "    cont_{}_drop(&{}container.instance);\n",
                    container_info.1.to_lowercase(),
                    this_access
                );
            }

            if context.2 {
                post_call += &format!(
                    "    ctx_{}_drop(&{}container.context);\n",
                    context.1.to_lowercase(),
                    this_access
                );
            }
        }

        let func_call = if self.calls_vtbl {
            format!(
                "({this_access}{vtbl})->{name}({call_args});\n",
                this_access = this_access,
                vtbl = vtbl,
                name = &self.name,
                call_args = call_args,
            )
        } else {
            "".to_string()
        };

        format!(
            r"
{inline} {return_type} {prefix}{name}({args}) {constness}{noexcept} {{
{ctx_clone}{func_result}{func_call}{post_call}{finish}
}}
",
            inline = if cpp_mode { "inline" } else { "static inline" },
            prefix = prefix,
            return_type = if wrap_in_container {
                this_ty
            } else {
                return_type
            },
            name = &self.name,
            args = args,
            constness = if self.moves_self && cpp_mode {
                "&& "
            } else if self.is_const && cpp_mode {
                "const "
            } else {
                ""
            },
            noexcept = if cpp_mode { "noexcept" } else { "" },
            ctx_clone = if self.moves_self && self.calls_vtbl {
                if cpp_mode {
                    "    auto ___ctx = StoreAll()[this->container.clone_context(), StoreAll()];\n"
                        .to_string()
                } else if context.2 {
                    format!(
                        "    {} ___ctx = ctx_{}_clone(&{}container.context);\n",
                        context.0,
                        context.1.to_lowercase(),
                        this_access
                    )
                } else {
                    String::new()
                }
            } else {
                String::new()
            },
            func_call = func_call,
            post_call = post_call,
            func_result = if wrap_in_container {
                format!(
                    r"    {} __ret;{}
    __ret.container = ",
                    this_ty, copied_vtbls
                )
            } else if return_type != "void" {
                format!("    {} __ret = ", return_type)
            } else {
                String::new()
            },
            finish = if return_type != "void" {
                "    return __ret;"
            } else {
                ""
            },
        )
    }
}

#[derive(Clone)]
pub struct Vtable {
    pub name: String,
    pub functions: Vec<Function>,
}

struct ArgsParser<'a> {
    args: &'a str,
}

impl<'a> Iterator for ArgsParser<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        if self.args.is_empty() {
            return None;
        }

        let mut brackets = [0, 0, 0];
        let mut ill_formed = false;

        let mut split = self.args.splitn(2, |c| match c {
            '<' => {
                brackets[0] += 1;
                false
            }
            '(' => {
                brackets[1] += 1;
                false
            }
            '[' => {
                brackets[2] += 1;
                false
            }
            '>' => {
                brackets[0] -= 1;
                if brackets[0] < 0 {
                    ill_formed = true;
                }
                false
            }
            ')' => {
                brackets[1] -= 1;
                if brackets[1] < 0 {
                    ill_formed = true;
                }
                false
            }
            ']' => {
                brackets[2] -= 1;
                if brackets[2] < 0 {
                    ill_formed = true;
                }
                false
            }
            ',' => !ill_formed && brackets == [0, 0, 0],
            _ => false,
        });

        let ret = split.next()?;

        self.args = split.next().unwrap_or("");

        if ill_formed {
            self.args = "";
            return None;
        }

        let name = ret.rsplit(&['&', '*', ' '][..]).next()?;

        Some((ret[..(ret.len() - name.len())].trim(), name.trim()))
    }
}

fn parse_arguments(args: &str) -> impl Iterator<Item = (&str, &str)> {
    ArgsParser { args }
}

impl Vtable {
    pub fn new(name: String, functions_str: &str, container_ty: &str) -> Result<Self> {
        let mut functions = vec![];

        // (?s) allows matching across lines
        let reg = Regex::new(&format!(
            r"(?s)(?P<ret_type>[^;]+)\(\*(?P<name>\w+)\)\((?P<cont>({cont_ty} \*|const {cont_ty} \*|{cont_ty} ))cont(?P<args>.*)\)",
            cont_ty = container_ty
        ))?;

        for func in functions_str.split(';').filter(|s| !s.is_empty()) {
            if let Some(cap) = reg.captures(func) {
                let cont = &cap["cont"];

                let mut arguments = vec![];

                let args = &cap["args"];

                if !args.is_empty() {
                    for (ty, name) in parse_arguments(&args[1..]) {
                        arguments.push(FunctionArg {
                            ty: ty.into(),
                            name: name.into(),
                        });
                    }
                }

                functions.push(Function {
                    name: cap["name"].to_string(),
                    return_type: cap["ret_type"].to_string(),
                    arguments,
                    is_const: cont.contains("const"),
                    moves_self: !cont.contains('*'),
                    calls_vtbl: true,
                });
            }
        }

        Ok(Self { name, functions })
    }

    pub fn create_wrappers(
        &self,
        cont_vtbl: (&str, &str),
        check_duplicates: impl Fn(&str) -> bool,
        (this_ty, vtbls): (&str, &[&str]),
    ) -> String {
        let mut ret = String::new();

        let regex = Regex::new(r"(\n)[^\S\r\n]+(\n|$)").unwrap();

        for f in &self.functions {
            let prefix = if check_duplicates(&f.name) {
                format!("{}_", self.name.to_lowercase())
            } else {
                String::new()
            };

            let wrapper = f.create_wrapper(
                cont_vtbl,
                &prefix,
                (true, false),
                (this_ty, vtbls),
                ("CGlueC", "", false),
                ("", "", false),
            );

            ret += &regex.replace_all(&wrapper.replace('\n', "\n    "), "$1$2");
        }

        ret
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_wrappers_c<'a>(
        &self,
        cont_vtbl: (&str, &str),
        (prefix, ty_prefix): (&str, &'a impl Fn(&Function) -> Option<&'a str>),
        container_info: (&str, &str, bool),
        context_info: (&str, &str, bool),
        (this_ty, vtbls): (&str, &[&str]),
        generated_funcs: &mut HashSet<(String, String)>,
        config: &Config,
    ) -> String {
        let mut ret = String::new();

        for f in self.functions.iter().chain(
            Some(Function {
                name: "drop".to_string(),
                return_type: "void".to_string(),
                arguments: vec![],
                is_const: false,
                moves_self: true,
                calls_vtbl: false,
            })
            .iter(),
        ) {
            let ty_prefix = ty_prefix(f);

            let (prefix, cast_self) = if f.moves_self || f.return_type == this_ty {
                let config_match = config.default_context.as_deref() == Some(context_info.1)
                    && config.default_container.as_deref() == Some(container_info.1);

                let ctx_prefix = if context_info.1.is_empty() || config_match {
                    String::new()
                } else {
                    format!("{}_", context_info.1.to_lowercase())
                };

                let container_prefix = if config_match {
                    String::new()
                } else {
                    format!("{}_", container_info.1.to_lowercase())
                };

                (
                    if let Some(ty) = ty_prefix {
                        format!(
                            "{}{}_{}{}",
                            prefix,
                            ty.to_lowercase(),
                            ctx_prefix,
                            container_prefix,
                        )
                    } else {
                        format!("{}{}{}", prefix, ctx_prefix, container_prefix,)
                    },
                    false,
                )
            } else if let Some(ty) = ty_prefix {
                (format!("{}{}_", prefix, ty.to_lowercase()), true)
            } else {
                (prefix.to_string(), true)
            };

            let prefix = if let Some(p) = config.function_prefix.as_deref() {
                format!("{}_{}", p, prefix)
            } else {
                prefix
            };

            if generated_funcs.insert((prefix.clone(), f.name.clone())) {
                ret += &f.create_wrapper(
                    cont_vtbl,
                    &prefix,
                    (false, cast_self),
                    (this_ty, vtbls),
                    container_info,
                    context_info,
                );
            }
        }

        ret
    }
}

#[derive(Clone)]
pub struct Group {
    pub name: String,
    pub vtables: Vec<(String, String)>,
}

impl Group {
    pub fn new(name: String, vtable_defs: &str) -> Result<Self> {
        let mut vtables = vec![];

        let reg = Regex::new(r"const (?P<vtbl_type>[^;]+)Vtbl.*;")?;

        for cap in reg.captures_iter(vtable_defs) {
            vtables.push((
                cap["vtbl_type"].to_string(),
                format!("vtbl_{}", cap["vtbl_type"].to_string().to_lowercase()),
            ));
        }

        Ok(Self { name, vtables })
    }

    pub fn create_wrappers(&self, vtables: &HashMap<&str, &Vtable>, container: &str) -> String {
        let mut ret = String::new();

        let mut vtbls = vec![];

        for (_, get) in &self.vtables {
            vtbls.push(get.as_str());
        }

        for (v, get) in &self.vtables {
            ret += &vtables.get(v.as_str()).unwrap().create_wrappers(
                (container, get),
                |name| {
                    self.vtables
                        .iter()
                        .filter_map(|(v, _)| vtables.get(v.as_str()))
                        .filter(|cv| &cv.name != v)
                        .any(|v| v.functions.iter().any(|f| f.name == name))
                },
                (&self.name, &vtbls),
            );
        }

        ret
    }
}
