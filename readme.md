# Sero-Operator

An operator made to control the [sero.rs](https://github.com/friespascal/sero-rs) project.

## How to use

1. Deploy the operator
2. annotate the namespace watched (or set the watched ns statically)
3. annotate the service based on #configuration
4. 🎉

| annotation | description | example | default |
|---|---|---|---|
| `beta.v1.sero/service` | name of the service routing to the deployment (not fqdn) | `cool-app` | `-` |
| `beta.v1.sero/inject` | whether sero should inject itself to the proxy (if you aren't sure use 'true') | `true` | `true` |
| `beta.v1.sero/timeout-forward` | the time Sero is waiting when forwarding in ms | `200` | `2000` |
| `beta.v1.sero/timeout-scaleup` | the time Sero is waiting for the service to scale up in ms | `8000` | `5000` |
| `beta.v1.sero/timeout-scale-down` | the time Sero is waiting for requests before scaling down in ms | `23000` | `15000` |

## Business use (license concerns)
If you would like to use or try the application in a business context and have concerns about the licence, please contact us directly.
