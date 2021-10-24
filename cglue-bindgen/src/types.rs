use itertools::Itertools;
use regex::*;
use std::collections::HashMap;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone)]
pub struct FunctionArg {
    ty: String,
    name: String,
}

#[derive(Clone)]
pub struct Function {
    name: String,
    return_type: String,
    arguments: Vec<FunctionArg>,
    is_const: bool,
    moves_self: bool,
}

impl Function {
    pub fn create_wrapper(&self, this: &str, vtbl: &str, this_ty: &str, vtbls: &[&str]) -> String {
        let args = self
            .arguments
            .iter()
            .map(|a| format!("{} {}", a.ty, a.name))
            .intersperse(", ".to_string())
            .collect::<String>();

        let mut call_args = String::new();

        for a in &self.arguments {
            call_args.push_str(", ");
            call_args.push_str(&a.name);
        }

        let mut copied_vtbls = String::new();

        let wrap_in_container = self.return_type.trim() == "CGlueC";

        if wrap_in_container {
            for v in vtbls {
                copied_vtbls.push_str(&format!("\n        __ret.{} = this->{};", v, v));
            }
        }

        format!(
            r"
    inline auto {name}({args}) {constness}noexcept {{
        {ctx_clone}{cont_forget}{vtbl_result} ({vtbl})->{name}({this_addr}{this}{call_args});{finish}
    }}
",
            name = &self.name,
            args = args,
            constness = if self.moves_self {
                "&& "
            } else if self.is_const {
                "const "
            } else {
                ""
            },
            this_addr = if self.moves_self { "" } else { "&" },
            ctx_clone = if self.moves_self {
                "auto ___ctx = StoreAll()[this->container.clone_context(), StoreAll()];\n        "
            } else {
                ""
            },
            cont_forget = if self.moves_self {
                "DeferedForget ___forget(this->container);\n        "
            } else {
                ""
            },
            this = this,
            vtbl = vtbl,
            call_args = call_args,
            vtbl_result = if wrap_in_container {
                format!(
                    r"{} __ret;{}
        __ret.container =",
                    this_ty, copied_vtbls
                )
            } else {
                "return".to_string()
            },
            finish = if wrap_in_container {
                "\n        return __ret;"
            } else {
                ""
            },
        )
    }
}

#[derive(Clone)]
pub struct Vtable {
    pub name: String,
    functions: Vec<Function>,
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
            ',' => {
                if !ill_formed && brackets == [0, 0, 0] {
                    true
                } else {
                    false
                }
            }
            _ => false,
        });

        let ret = split.next()?;

        self.args = split.next().unwrap_or("");

        if ill_formed {
            self.args = "";
            return None;
        }

        let name = ret.rsplit(&['&', '*', ' '][..]).next()?;

        Some((&ret[..(ret.len() - name.len())].trim(), name.trim()))
    }
}

fn parse_arguments(args: &str) -> impl Iterator<Item = (&str, &str)> {
    ArgsParser { args }
}

impl Vtable {
    pub fn new(name: String, functions_str: &str) -> Result<Self> {
        let mut functions = vec![];

        let reg = Regex::new(
            r"(?P<ret_type>[^;]+)\(\*(?P<name>\w+)\)\((?P<cont>(CGlueC \*|const CGlueC \*|CGlueC ))cont(?P<args>.*)\)",
        )?;

        for func in functions_str.split(";").filter(|s| !s.is_empty()) {
            //println!("'{}'", func);
            if let Some(cap) = reg.captures(&func) {
                let cont = &cap["cont"];

                let mut arguments = vec![];

                let args = &cap["args"];

                if args.len() > 0 {
                    for (ty, name) in parse_arguments(&args[1..]) {
                        //println!("ARG: {} | {}", ty, name);
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
                    moves_self: !cont.contains("*"),
                });
                //println!("{} | {} | {} | {}", &cap["ret_type"], &cap["name"], &cap["cont"], &cap["args"]);
            }
        }

        Ok(Self { name, functions })
    }

    pub fn create_wrappers(
        &self,
        container: &str,
        vtbl: &str,
        this_ty: &str,
        vtbls: &[&str],
    ) -> String {
        let mut ret = String::new();

        for f in &self.functions {
            ret += &f.create_wrapper(container, vtbl, this_ty, vtbls);
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
            ret += &vtables
                .get(v.as_str())
                .unwrap()
                .create_wrappers(container, get, &self.name, &vtbls);
        }

        ret
    }
}

#[derive(Clone)]
pub struct Header {
    vtables: HashMap<String, Vtable>,
    groups: Vec<Group>,
}
