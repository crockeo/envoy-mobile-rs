package main

import (
	"net/http"
)

func handler(w http.ResponseWriter, r *http.Request) {
	w.WriteHeader(http.StatusOK)
	w.Write([]byte("hello"))
}

func main() {
	http.HandleFunc("/", handler);
	http.ListenAndServe("0.0.0.0:8080", nil);
}
