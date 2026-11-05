# RFC sources

`rfc9110.xml` (RFC 9110, HTTP Semantics) and `rfc9112.xml` (RFC 9112, HTTP/1.1) are RFCs
published by the RFC Editor, in their RFCXML v3 form, unmodified. They are subject to the IETF
Trust Legal Provisions relating to IETF Documents
(<https://trustee.ietf.org/license-info>), whose copyright notice each file carries.

| file | from | bytes | sha256 |
|---|---|---|---|
| `rfc9110.xml` | <https://www.rfc-editor.org/rfc/rfc9110.xml> | 1,223,686 | `09ec9ad4d1664623adcb298577e3fb9cd5c285f1bbe6b24d8c715b29f218e721` |
| `rfc9112.xml` | <https://www.rfc-editor.org/rfc/rfc9112.xml> | 262,821 | `ce82219c272374005115c909244a79e587687df3858edca4112b8563c02ccf19` |

They are vendored so that `ignite rfc check` can compare the catalogues
`std/rfc/rfc9110.ignite` and `std/rfc/rfc9112.ignite` with their sources offline, in the gate.
Each catalogue's header names the `ignite rfc import` command that wrote it from these files.

RFC 854, RFC 855 and RFC 1143 predate the Trust's provisions and are cited, not vendored: the
catalogues `std/rfc/rfc854.ignite`, `rfc855.ignite` and `rfc1143.ignite` quote the sentences
they need with page or section citations and record the sha256 of the published text file,
which `ignite rfc check` without a source reports as `unchecked` (`docs/slice.md` 19.9, D5).
