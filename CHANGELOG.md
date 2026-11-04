# Changelog

## [0.2.0](https://github.com/IdleCharm/nanit-proxy/compare/nanit-proxy-v0.1.0...nanit-proxy-v0.2.0) (2026-11-04)


### Features

* initial release of nanit-proxy control plane & telemetry daemon ([b63adf1](https://github.com/IdleCharm/nanit-proxy/commit/b63adf1b334a4d8b7a37c0fa697489b3cc99a3c4))


### Bug Fixes

* **api:** correct token refresh route from /login/refresh to /tokens/refresh ([962e7b4](https://github.com/IdleCharm/nanit-proxy/commit/962e7b4efcb6c2f3f7981b1a109578601efecfff))
* **audit:** resolve security vulnerabilities, update GitHub username to IdleCharm, and convert architecture diagram to mermaid ([353623a](https://github.com/IdleCharm/nanit-proxy/commit/353623abab1408c884a0e7d531485dde166bbe4f))
* **auth:** support HTTP 482 status code for Nanit MFA challenge ([2611336](https://github.com/IdleCharm/nanit-proxy/commit/26113361a07bd24e9b68ea43e87a5ee6676fa11d))
* **ws:** use IntoClientRequest to populate required Sec-WebSocket-Key header ([e3f8eae](https://github.com/IdleCharm/nanit-proxy/commit/e3f8eae78b9801b56e0410c6f5f745848fb3c7f4))
