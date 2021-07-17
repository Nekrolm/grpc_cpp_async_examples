
import logging
import asyncio
import grpc
import argparse

import bidi_hello_pb2
import bidi_hello_pb2_grpc


def parse_args():
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--list", action='store_true')
    group.add_argument("--connect-to", type=str, default=None)
    return parser.parse_args()

async def communicate_stream(name):
    yield bidi_hello_pb2.HelloRequest(selector=bidi_hello_pb2.Selector(service_name=name))
    for i in range(100):
        await asyncio.sleep(0.1)
        yield bidi_hello_pb2.HelloRequest(data=bidi_hello_pb2.DataChunk(text="Hello from python!"))

async def run() -> None:
    args = parse_args()
    async with grpc.aio.insecure_channel('127.0.0.1:8080') as channel:
        stub = bidi_hello_pb2_grpc.BidiHelloServiceStub(channel)
        if args.list:
            print(await stub.ListAvailableServices(bidi_hello_pb2.Empty()))
        else:
            async for line in stub.Communicate(communicate_stream(args.connect_to)):
                print(line)


if __name__ == '__main__':
    logging.basicConfig()
    asyncio.run(run())