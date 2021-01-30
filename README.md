# Tunneler
Tunneler is a simple reverse proxy/tunnel application that is meant to be used to get traffic from a public server into your non public server/network.

## Creating a Tunnel
To create a tunnel that "forwards" all requests from port 80 on the public server to port 80 on the local machine:
* Start the server-side using `tunneler server -p 80 -l 81 --key [the key]`
* Start the client-side using `tunneler client -p 80 -l 81 --ip [ip of public server] --key [the key]`
