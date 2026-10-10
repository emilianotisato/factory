package client

import "net/http"

// Client talks to the API at base.
type Client struct {
	base string
	http *http.Client
}

// NewClient builds a Client for base.
func NewClient(base string) *Client {
	return &Client{base: base, http: http.DefaultClient}
}

// Get fetches path under base.
func (c *Client) Get(path string) (*http.Response, error) {
	return c.http.Get(c.base + path)
}
