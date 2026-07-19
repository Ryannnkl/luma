Name:           lumalock
Version:        0.2.0
Release:        1%{?dist}
Summary:        Secure and customizable Wayland session locker

License:        MIT AND Apache-2.0 AND OFL-1.1 AND Ubuntu-font-1.0 AND Unicode-3.0 AND CC0-1.0 AND ISC AND MPL-2.0 AND Zlib
URL:            https://github.com/Ryannnkl/luma
Source0:        %{url}/archive/refs/tags/v%{version}.tar.gz
Source1:        %{url}/releases/download/v%{version}/luma-%{version}-vendor.tar.xz

ExclusiveArch:  x86_64

BuildRequires:  cargo >= 1.92
BuildRequires:  cargo-rpm-macros
BuildRequires:  gcc
BuildRequires:  rust >= 1.92
BuildRequires:  pkgconfig(pam)
BuildRequires:  pkgconfig(xkbcommon)
Requires:       pam

%description
Luma is a secure, customizable Wayland session locker built around the
ext-session-lock-v1 protocol. It provides PAM authentication, multi-output
coverage, configurable clock and input rendering, and optional blurred
background capture.

%prep
%autosetup -n luma-%{version} -a 1
%cargo_prep -v vendor

%build
%cargo_build

%check
%cargo_test

%install
install -Dpm0755 target/rpm/luma %{buildroot}%{_bindir}/luma
install -Dpm0644 pam/luma %{buildroot}%{_sysconfdir}/pam.d/luma

%files
%license LICENSE cargo-vendor.txt
%doc README.md config.example.toml
%{_bindir}/luma
%config(noreplace) %{_sysconfdir}/pam.d/luma

%changelog
* Sun Jul 19 2026 Ryannnkl <ryannnkl@gmail.com> - 0.2.0-1
- Initial package
