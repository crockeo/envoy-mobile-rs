package main

import (
	"fmt"
	"net/http"
)

func handler(w http.ResponseWriter, r *http.Request) {
	for key, value := range r.Header {
		fmt.Printf("%v: %v\n", key, value);
	}
	w.WriteHeader(http.StatusOK)
	w.Write([]byte("hello"))
}

func main() {
	http.HandleFunc("/", handler);
	http.ListenAndServe("0.0.0.0:8080", nil);
}
