import asyncio
from uuid import uuid4
from taurino import Builder, Config, IpcConnectionConfig, App, AppHandle

builder = Builder(
    Config(
        productname="My App",
        version="0.1.0",
        app_id=uuid4(),
        connection=IpcConnectionConfig(endpoint=f"taurino-{uuid4().hex}"),
    )
)


@builder.setup()
def setup(app: App) -> None:
    # create State and optionally call app.configure(new_config)
    app.state.set({"initialized": True})


@builder.command("greet")
async def greet(name: str, handle: AppHandle) -> str:
    return f"Hello, {name}!"


@builder.on_event()  # alternatively @builder.run()
async def event_loop_hook(handle: AppHandle, event: dict) -> None:
    print(event)


async def main() -> None:
    app = builder.build()  # default requires taurino.engine.runtime (compiled PyO3)
    async with app:
        res = await app.handle.call("engine.ping", result_type=str)
        if res == "pong":
            pass
        
        print(res)


if __name__ == "__main__":
    asyncio.run(main())
