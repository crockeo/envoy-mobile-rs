import envoy_mobile


def main():
    response = envoy_mobile.request("GET", "https://api.lyft.com/ping", None, None)
    print(response.status_code)
    print(response.headers)
    print(response.body)


if __name__ == "__main__":
    main()
