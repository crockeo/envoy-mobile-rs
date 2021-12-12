from gevent.monkey import patch_all
patch_all()

import envoy_requests

def main():
    while True:
        res = envoy_requests.get("https://api.lyft.com/ping")
        print(res)


if __name__ == "__main__":
    main()
