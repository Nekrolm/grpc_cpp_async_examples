
import logging
import asyncio
import grpc

import hello_stream_pb2
import hello_stream_pb2_grpc


async def run() -> None:
    async with grpc.aio.insecure_channel('127.0.0.1:8081') as channel:
        stub = hello_stream_pb2_grpc.HelloServiceStub(channel)
        async for line in stub.OutputStream(hello_stream_pb2.Empty()):
            print(line)


if __name__ == '__main__':
    logging.basicConfig()
    asyncio.run(run())