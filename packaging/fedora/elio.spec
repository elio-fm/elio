%bcond_with check
%global fallback_version 1.5.0
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
BuildRequires:  desktop-file-utils
Requires:       hicolor-icon-theme

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
install -Dpm0644 packaging/linux/%{name}.desktop %{buildroot}%{_datadir}/applications/%{name}.desktop
for size in 48 128 256 512; do
    install -Dpm0644 packaging/linux/icons/hicolor/${size}x${size}/apps/%{name}.png %{buildroot}%{_datadir}/icons/hicolor/${size}x${size}/apps/%{name}.png
done

%check
desktop-file-validate packaging/linux/%{name}.desktop
%if %{with check}
%cargo_test
%endif

%files
%license LICENSE-MIT
%doc README.md CHANGELOG.md
%{_bindir}/elio
%{_datadir}/applications/%{name}.desktop
%{_datadir}/icons/hicolor/*/apps/%{name}.png

%changelog
* Thu May 14 2026 Miguel Regueiro <miguelpr4242@gmail.com> - 1.5.0-1
- Add zoxide directory jumps
- Add transparent theme color values and chip text palette control
- Improve image previews inside tmux with automatic passthrough setup and expanded Sixel, iTerm inline, and Kitty direct-placement handling
- Fix Windows Terminal Sixel preview sizing on WSL
- Improve bundled light-theme chip contrast

* Sun May 03 2026 Miguel Regueiro <miguelpr4242@gmail.com> - 1.4.0-1
- Add startup directory argument
- Add keyboard preview scrolling and updated preview controls
- Fix Linux/BSD default app dispatch, Warp previews, and tmux Kitty passthrough

* Tue Apr 28 2026 Miguel Regueiro <miguelpr4242@gmail.com> - 1.3.0-1
- Add Konsole inline image preview support
- Fix crates.io installs by upgrading lofty to 0.24

* Sat Apr 25 2026 Miguel Regueiro <miguelpr4242@gmail.com> - 1.2.0-1
- Add QML preview support and icon coverage
- Add font metadata previews
- Improve large-directory navigation responsiveness
- Fix browser license icons and truncated directory preview totals

* Sat Apr 18 2026 Miguel Regueiro <miguelpr4242@gmail.com> - 1.1.0-1
- Add RAR, comic archive, and Kindle ebook previews
- Improve fuzzy search refresh and large-tree responsiveness
- Clarify metadata preview sections
- Improve trash handling and documentation

* Sun Apr 12 2026 Miguel Regueiro <miguelpr4242@gmail.com> - 1.0.1-1
- Add CLI help and version flags
- Add release packaging automation

* Sat Apr 11 2026 Miguel Regueiro <miguelpr4242@gmail.com> - 1.0.0-1
- Initial COPR package
