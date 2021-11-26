package main

import (
	"log"
	"net/http"
	"time"
)

func handler(w http.ResponseWriter, r *http.Request) {
	log.Println("recv");
	time.Sleep(time.Second * 1)
	written, err := w.Write([]byte("hello"))
	log.Println("resp", written, err)
}

func main() {
	http.HandleFunc("/", handler);
	http.ListenAndServe("0.0.0.0:8080", nil);
}
