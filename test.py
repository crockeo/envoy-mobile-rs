import envoy_mobile


def main():
    engine = envoy_mobile.Engine(envoy_mobile.LogLevel.Debug)
    engine.terminate()


if __name__ == "__main__":
    main()
