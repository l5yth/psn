# Copyright (c) 2026 l5yth
# SPDX-License-Identifier: Apache-2.0

EAPI=8

inherit cargo git-r3

DESCRIPTION="Terminal UI for process status navigation and control"
HOMEPAGE="https://github.com/l5yth/psn"
EGIT_REPO_URI="https://github.com/l5yth/psn.git"

LICENSE="Apache-2.0"
SLOT="0"
KEYWORDS=""
IUSE=""
PROPERTIES="live"


BDEPEND="
	dev-lang/rust
"

src_unpack() {
	git-r3_src_unpack
	cargo_live_src_unpack
}

src_compile() {
	cargo_src_compile --bin psn
}

src_install() {
	dobin target/release/psn
	einstalldocs
}
