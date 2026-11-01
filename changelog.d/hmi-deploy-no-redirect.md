### Security

- Deploy and Roll back in the HMI Builder follow no redirect from the runtime's web server, so the runtime's control
  token is sent only to that web server, as in Live mode (specification 39 §16.2 item 8). Before, the extension
  followed a redirect with the token.
