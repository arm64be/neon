use mlua::{Lua, Result, UserData, UserDataMethods};
use tachyonfx::{dsl::EffectDsl, Effect};

fn rt_err(err: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::RuntimeError(err.to_string())
}

struct TachyonFxDsl {
    dsl: EffectDsl,
}

impl TachyonFxDsl {
    fn new() -> Self {
        Self {
            dsl: EffectDsl::new(),
        }
    }

    fn compile(&self, source: String) -> Result<TachyonFxEffect> {
        let effect = self.dsl.compiler().compile(&source).map_err(rt_err)?;
        Ok(TachyonFxEffect {
            effect,
            source: Some(source),
        })
    }
}

impl UserData for TachyonFxDsl {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("compile", |_, this, source: String| this.compile(source));
    }
}

struct TachyonFxEffect {
    effect: Effect,
    source: Option<String>,
}

impl UserData for TachyonFxEffect {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("name", |_, this, ()| Ok(this.effect.name().to_string()));
        methods.add_method("done", |_, this, ()| Ok(this.effect.done()));
        methods.add_method("running", |_, this, ()| Ok(this.effect.running()));
        methods.add_method("source", |_, this, ()| Ok(this.source.clone()));
        methods.add_method("clone", |_, this, ()| {
            Ok(TachyonFxEffect {
                effect: this.effect.clone(),
                source: this.source.clone(),
            })
        });
    }
}

pub fn create_module(lua: &Lua) -> Result<mlua::Table> {
    let module = lua.create_table()?;
    module.set("new", lua.create_function(|_, ()| Ok(TachyonFxDsl::new()))?)?;
    module.set(
        "compile",
        lua.create_function(|_, source: String| TachyonFxDsl::new().compile(source))?,
    )?;
    module.set("available", true)?;
    module.set("codename", "tachyonfx")?;
    module.set("version", env!("CARGO_PKG_VERSION"))?;
    Ok(module)
}
