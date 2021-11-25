import envoy_mobile


def main():
    print("starting...")
    engine = envoy_mobile.Engine(envoy_mobile.LogLevel.Debug)

    print("creating stream...")
    try:
        stream = engine.new_stream(False)
        # TODO: something here
    except BaseException:
        pass

    print("terminating...")
    engine.terminate()


if __name__ == "__main__":
    main()
