#![allow(clippy::missing_errors_doc)]

use std::time::Duration;

use async_io::{block_on, Timer};

use mlua::prelude::*;
use mlua_luau_runtime::Runtime;

const MAIN_SCRIPT: &str = include_str!("./lua/lots_of_threads.luau");

const ONE_NANOSECOND: Duration = Duration::from_nanos(1);

pub fn main() -> LuaResult<()> {
    tracing_subscriber::fmt::init();

    // Set up persistent Lua environment
    let lua = Lua::new();
    let rt = Runtime::new(&lua);

    let rt_fns = rt.create_functions()?;
    lua.globals().set("spawn", rt_fns.spawn)?;
    lua.globals().set(
        "sleep",
        lua.create_async_function(|_, ()| async move {
            // Obviously we can't sleep for a single nanosecond since
            // this uses OS scheduling under the hood, but we can try
            Timer::after(ONE_NANOSECOND).await;
            Ok(())
        })?,
    )?;

    // Load the main script into the runtime
    let main = lua.load(MAIN_SCRIPT);
    rt.push_thread_front(main, ())?;

    // Run until completion
    block_on(rt.run());

    Ok(())
}

#[test]
fn test_lots_of_threads() -> LuaResult<()> {
    main()
}
