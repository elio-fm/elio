%bcond_with check
%global fallback_version 1.0.1
%global fallback_release 1

Name:           elio
Version:        %{?elio_version}%{!?elio_version:%{fallback_version}}
Release:        %{?elio_release}%{!?elio_release:%{fallback_release}}%{?dist}
Summary:        Terminal-native file manager with rich previews and inline images

License:        MIT
URL:            https://github.com/elio-fm/elio
Source0:        %{name}-%{version}.tar.gz
Source1:        vendor-%{version}.tar.zst

BuildRequires:  cargo-rpm-macros
BuildRequires:  cargo >= 1.93
BuildRequires:  rust >= 1.93
BuildRequires:  gcc
BuildRequires:  pkgconf-pkg-config
BuildRequires:  zstd

%description
elio is a terminal-native file manager with a three-pane layout, rich previews,
inline images, customizable Places, trash support, and quick actions.

%prep
%autosetup -a 1
%cargo_prep -v vendor

%build
%cargo_build

%install
install -Dpm0755 target/rpm/%{name} %{buildroot}%{_bindir}/%{name}

%check
%if %{with check}
%cargo_test
%endif

%files
%license LICENSE-MIT
%doc README.md CHANGELOG.md
%{_bindir}/elio

%changelog
* Sun Apr 12 2026 Miguel Regueiro <miguelpr4242@gmail.com> - 1.0.1-1
- Add CLI help and version flags
- Add release packaging automation

* Sat Apr 11 2026 Miguel Regueiro <miguelpr4242@gmail.com> - 1.0.0-1
- Initial COPR package
