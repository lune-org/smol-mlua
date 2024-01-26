use std::sync::Arc;

use concurrent_queue::ConcurrentQueue;
use mlua::prelude::*;
use smol::channel::{unbounded, Receiver, Sender};

use crate::IntoLuaThread;

const ERR_OOM: &str = "out of memory";

/**
    Queue for storing [`LuaThread`]s with associated arguments.

    Provides methods for pushing and draining the queue, as
    well as listening for new items being pushed to the queue.
*/
#[derive(Debug, Clone)]
pub struct ThreadQueue {
    queue: Arc<ConcurrentQueue<ThreadWithArgs>>,
    signal_tx: Sender<()>,
    signal_rx: Receiver<()>,
}

impl ThreadQueue {
    pub fn new() -> Self {
        let queue = Arc::new(ConcurrentQueue::unbounded());
        let (signal_tx, signal_rx) = unbounded();
        Self {
            queue,
            signal_tx,
            signal_rx,
        }
    }

    pub fn push<'lua>(
        &self,
        lua: &'lua Lua,
        thread: impl IntoLuaThread<'lua>,
        args: impl IntoLuaMulti<'lua>,
    ) -> LuaResult<()> {
        let thread = thread.into_lua_thread(lua)?;
        let args = args.into_lua_multi(lua)?;
        let stored = ThreadWithArgs::new(lua, thread, args);

        self.queue.push(stored).unwrap();
        self.signal_tx.try_send(()).unwrap();

        Ok(())
    }

    pub fn drain<'outer, 'lua>(
        &'outer self,
        lua: &'lua Lua,
    ) -> impl Iterator<Item = (LuaThread<'lua>, LuaMultiValue<'lua>)> + 'outer
    where
        'lua: 'outer,
    {
        self.queue.try_iter().map(|stored| stored.into_inner(lua))
    }

    pub async fn listen(&self) {
        self.signal_rx.recv().await.unwrap();
        // Drain any pending receives
        loop {
            match self.signal_rx.try_recv() {
                Ok(_) => continue,
                Err(_) => break,
            }
        }
    }
}

/**
    Representation of a [`LuaThread`] with associated arguments currently stored in the Lua registry.
*/
#[derive(Debug)]
struct ThreadWithArgs {
    key_thread: LuaRegistryKey,
    key_args: LuaRegistryKey,
}

impl ThreadWithArgs {
    pub fn new<'lua>(lua: &'lua Lua, thread: LuaThread<'lua>, args: LuaMultiValue<'lua>) -> Self {
        let argsv = args.into_vec();

        let key_thread = lua.create_registry_value(thread).expect(ERR_OOM);
        let key_args = lua.create_registry_value(argsv).expect(ERR_OOM);

        Self {
            key_thread,
            key_args,
        }
    }

    pub fn into_inner(self, lua: &Lua) -> (LuaThread<'_>, LuaMultiValue<'_>) {
        let thread = lua.registry_value(&self.key_thread).unwrap();
        let argsv = lua.registry_value(&self.key_args).unwrap();

        let args = LuaMultiValue::from_vec(argsv);

        lua.remove_registry_value(self.key_thread).unwrap();
        lua.remove_registry_value(self.key_args).unwrap();

        (thread, args)
    }
}
