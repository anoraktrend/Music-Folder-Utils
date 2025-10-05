# Fast Build Justfile for mfutil
#
# Just is a modern command runner like Make but with better features.
# Install it with: cargo install just
#
# Usage:
#   just build          # Fast optimized build
#   just dev            # Fast development build
#   just native         # Native target build
#   just check          # Check without building
#   just test           # Run tests
#   just clean          # Clean build artifacts
#   just install-deps   # Install system dependencies
#   just install-sccache # Install build cache
#   just install-system                    # Install to /usr/local/bin
#   just install-local  # Install for current user
#   just install-custom /path/to/dir # Install to custom location
#   just install-native-system      # Install native build system-wide
#   just benchmark      # Compare build times
#   just help           # Show this help

# Default recipe - show help
default:
    @just --list

# Fast development build (fastest option)
dev:
    #!/usr/bin/env bash
    echo "Fast development build..."
    echo "Using all CPU cores: $(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4) jobs"

    # Check if sccache is available (build cache)
    if command -v sccache &> /dev/null; then
        echo "Using sccache for faster builds..."
        export RUSTC_WRAPPER=sccache
    fi

    # Enable incremental compilation for much faster dev builds
    export CARGO_INCREMENTAL=1

    time cargo build -j $(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)

    echo "Dev build completed!"
    echo "Binary size: $(du -h target/debug/mfutil 2>/dev/null | cut -f1 || echo 'unknown')"
    echo "Ready to run: ./target/debug/mfutil"

# Optimized release build
build:
    #!/usr/bin/env bash
    echo "Optimized release build..."
    echo "Using all CPU cores: $(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4) jobs"

    # Check if sccache is available (build cache)
    if command -v sccache &> /dev/null; then
        echo "Using sccache for faster builds..."
        export RUSTC_WRAPPER=sccache
    fi

    # Check if lld is available (faster linker) - try both lld and ld.lld
    if command -v ld.lld &> /dev/null; then
        echo "Using ld.lld for faster linking..."
        export RUSTFLAGS="$RUSTFLAGS -C link-arg=-fuse-ld=lld"
    elif command -v lld &> /dev/null; then
        echo "Using lld for faster linking..."
        export RUSTFLAGS="$RUSTFLAGS -C link-arg=-fuse-ld=lld"
    fi

    time cargo build --release -j $(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)

    echo "Build completed successfully!"
    echo "Binary size: $(du -h target/release/mfutil 2>/dev/null | cut -f1 || echo 'unknown')"
    echo "Ready to run: ./target/release/mfutil"

# Fast release build (no LTO, faster but larger binary)
build-fast:
    #!/usr/bin/env bash
    echo "Fast release build (no LTO)..."
    echo "Using all CPU cores: $(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4) jobs"

    # Check if sccache is available (build cache)
    if command -v sccache &> /dev/null; then
        echo "Using sccache for faster builds..."
        export RUSTC_WRAPPER=sccache
    fi

    # Check if lld is available (faster linker) - try both lld and ld.lld
    if command -v ld.lld &> /dev/null; then
        echo "Using ld.lld for faster linking..."
        export RUSTFLAGS="$RUSTFLAGS -C link-arg=-fuse-ld=lld"
    elif command -v lld &> /dev/null; then
        echo "Using lld for faster linking..."
        export RUSTFLAGS="$RUSTFLAGS -C link-arg=-fuse-ld=lld"
    fi

    # Use faster release profile without LTO
    export RUSTFLAGS="$RUSTFLAGS -C opt-level=3 -C codegen-units=256"
    time cargo build --release -j $(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)

    echo "Fast build completed!"
    echo "Binary size: $(du -h target/release/mfutil 2>/dev/null | cut -f1 || echo 'unknown')"
    echo "Ready to run: ./target/release/mfutil"

# Native target build
native:
    #!/usr/bin/env bash
    echo "Native target build..."
    echo "Using all CPU cores: $(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4) jobs"

    # Check if sccache is available (build cache)
    if command -v sccache &> /dev/null; then
        echo "Using sccache for faster builds..."
        export RUSTC_WRAPPER=sccache
    fi

    # Set optimization level
    export RUSTFLAGS="$RUSTFLAGS -C opt-level=3"

    # Check if lld is available (faster linker) - try both lld and ld.lld
    if command -v ld.lld &> /dev/null; then
        echo "Using ld.lld for faster linking..."
        export RUSTFLAGS="$RUSTFLAGS -C link-arg=-fuse-ld=lld"
    elif command -v lld &> /dev/null; then
        echo "Using lld for faster linking..."
        export RUSTFLAGS="$RUSTFLAGS -C link-arg=-fuse-ld=lld"
    fi

    HOST_TARGET=$(rustc --print host-tuple)
    echo "Building for native target: $HOST_TARGET"
    time cargo build --release --target $HOST_TARGET -j $(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)

    echo "Native build completed successfully!"
    echo "Binary size: $(du -h target/$HOST_TARGET/release/mfutil 2>/dev/null | cut -f1 || echo 'unknown')"
    echo "Ready to run: ./target/$HOST_TARGET/release/mfutil"

# Check code without building
check:
    #!/usr/bin/env bash
    echo "Checking code..."
    cargo check

# Run tests
test:
    #!/usr/bin/env bash
    echo "Running tests..."
    cargo test

# Clean build artifacts
clean:
    #!/usr/bin/env bash
    echo "Cleaning build artifacts..."
    cargo clean

# Install system dependencies for mfutil
install-deps:
	#!/usr/bin/env bash
	echo "Installing system dependencies for mfutil..."
	echo "This requires administrator privileges"

	# Detect available privilege escalation tools
	PRIV_ESC=""
	if command -v doas &> /dev/null; then
		if doas true 2>/dev/null; then
			PRIV_ESC="doas"
			echo "Using doas for privilege escalation"
		else
			echo "doas installed but not configured, skipping..."
		fi
	fi
	if [ -z "$PRIV_ESC" ] && command -v sudo &> /dev/null; then
		if sudo true 2>/dev/null; then
			PRIV_ESC="sudo"
			echo "Using sudo for privilege escalation"
		else
			echo "sudo installed but not configured, skipping..."
		fi
	fi
	if [ -z "$PRIV_ESC" ]; then
		echo "No configured privilege escalation tool found"
		echo "Please install dependencies manually for your system"
		exit 1
	fi

	# Check if we're on Linux
	if [[ "$OSTYPE" == "linux-gnu"* ]]; then
		# Detect Linux distribution
		if [ -f /etc/os-release ]; then
			. /etc/os-release
			DISTRO=$ID
		elif command -v lsb_release &> /dev/null; then
			DISTRO=$(lsb_release -si | tr '[:upper:]' '[:lower:]')
		else
			DISTRO="unknown"
		fi

		echo "Detected Linux distribution: $DISTRO"

		case $DISTRO in
			ubuntu|debian|pop|elementary|linuxmint|zorin)
				echo "Using apt (Debian/Ubuntu family)..."
				echo "Updating package list..."
				$PRIV_ESC apt update
				echo "Installing dependencies..."
				$PRIV_ESC apt install -y build-essential pkg-config libavformat-dev libavcodec-dev libavutil-dev libmagickwand-dev libglib2.0-dev libgirepository1.0-dev git ffmpeg clang llvm lld
				;;
			arch|manjaro|endeavouros|garuda|cachyos)
				echo "Using pacman (Arch Linux family)..."
				$PRIV_ESC pacman -Syu --noconfirm
				$PRIV_ESC pacman -S --needed --noconfirm base-devel ffmpeg imagemagick glib2 gobject-introspection pkgconf git clang llvm lld
				;;
			fedora)
				echo "Using dnf (Fedora)..."
				$PRIV_ESC dnf check-update
				$PRIV_ESC dnf install -y @development-tools ffmpeg-devel ImageMagick-devel glib2-devel gobject-introspection-devel pkgconf git clang llvm lld
				;;
			centos|rhel|almalinux|rocky)
				if command -v dnf &> /dev/null; then
					echo "Using dnf (CentOS/RHEL 8+)..."
					$PRIV_ESC dnf check-update
					$PRIV_ESC dnf install -y @development-tools ffmpeg-devel ImageMagick-devel glib2-devel gobject-introspection-devel pkgconf git clang llvm lld
				else
					echo "Using yum (CentOS/RHEL 7)..."
					$PRIV_ESC yum check-update
					$PRIV_ESC yum install -y @development-tools ffmpeg-devel ImageMagick-devel glib2-devel gobject-introspection-devel pkgconfig git clang llvm lld
				fi
				;;
			opensuse*|sles)
				echo "Using zypper (openSUSE)..."
				$PRIV_ESC zypper refresh
				$PRIV_ESC zypper install -y -t pattern devel_basis
				$PRIV_ESC zypper install -y ffmpeg libMagickWand imagemagick glib2-devel gobject-introspection-devel pkg-config git clang llvm lld
				;;
			gentoo)
				echo "Using emerge (Gentoo)..."
				$PRIV_ESC emerge --sync
				$PRIV_ESC emerge -av dev-build/pkgconf media-video/ffmpeg media-gfx/imagemagick dev-libs/glib dev-libs/gobject-introspection dev-vcs/git sys-devel/clang sys-devel/llvm sys-devel/lld
				;;
			slackware)
				echo "Using slackpkg (Slackware)..."
				echo "Slackware requires manual dependency installation."
				echo "Please install: gcc, make, pkg-config, ffmpeg, ImageMagick, glib2, gobject-introspection, git, clang, llvm, lld"
				echo "Using slackpkg or sbopkg"
				exit 1
				;;
			*)
				echo "Unsupported or unknown Linux distribution: $DISTRO"
				echo "Please install dependencies manually."
				echo "Required packages: build tools, pkg-config, ffmpeg, ImageMagick, glib2, gobject-introspection, git, clang, llvm, lld"
				echo ""
				echo "For your system, try one of these:"
				echo "  Ubuntu/Debian: sudo apt install build-essential pkg-config libavformat-dev libavcodec-dev libavutil-dev libmagickwand-dev libglib2.0-dev libgirepository1.0-dev git ffmpeg clang llvm lld"
				echo "  Arch Linux: sudo pacman -S base-devel ffmpeg imagemagick glib2 gobject-introspection pkgconf git clang llvm lld"
				echo "  Fedora: sudo dnf install @development-tools ffmpeg-devel ImageMagick-devel glib2-devel gobject-introspection-devel pkgconf git clang llvm lld"
				echo "  openSUSE: sudo zypper install -t pattern devel_basis && sudo zypper install ffmpeg libMagickWand imagemagick glib2-devel gobject-introspection-devel pkg-config git clang llvm lld"
				exit 1
				;;
		esac

		echo "Dependencies installed successfully!"
	else
		echo "Automatic dependency installation only supported on Linux"
		echo "Please install dependencies manually for your system"
		echo "Required packages: build tools, pkg-config, ffmpeg, ImageMagick, glib2, gobject-introspection, git"
	fi

# Install all build dependencies and tools
setup: install-deps install-sccache
	#!/usr/bin/env bash
	echo "All build dependencies and tools installed!"
	echo "You can now run 'just dev' for fast development builds"

# Install sccache for build caching
install-sccache:
	#!/usr/bin/env bash
	echo "Installing sccache..."
	cargo install sccache

# Benchmark different build methods
benchmark:
    #!/usr/bin/env bash
    echo "Benchmarking different build methods..."
    echo "=================================================="

    # Clean any existing builds
    cargo clean > /dev/null 2>&1

    echo ""
    echo "Testing standard dev build..."
    time cargo build > /dev/null 2>&1

    echo ""
    echo "Testing optimized dev build..."
    time just dev > /dev/null 2>&1

    echo ""
    echo "Testing standard release build..."
    time cargo build --release > /dev/null 2>&1

    echo ""
    echo "Testing native target build..."
    time just native > /dev/null 2>&1

    echo ""
    echo "Testing optimized release build..."
    time just build > /dev/null 2>&1

    echo ""
    echo "Recommendations:"
    echo "• Use 'just dev' for development (fastest)"
    echo "• Use 'just build' for releases (optimized)"
    echo "• Use 'just check' to verify without building"

# Show build optimization tips
tips:
    #!/usr/bin/env bash
    echo "Build Optimization Tips:"
    echo ""
    echo "Quick Commands:"
    echo "  just dev     - Fastest development builds"
    echo "  just build   - Optimized release builds"
    echo "  just native  - Native target build"
    echo "  just build-fast - Fast release builds (no LTO)"
    echo "  just check   - Check without building"
    echo "  just test    - Run tests"
    echo ""
    echo "Installation Commands:"
    echo "  just install-deps       - Install system dependencies"
    echo "  just install-local  - Install for current user ($HOME/.local/bin)"
    echo "  just install-system - Install system-wide (/usr/local/bin)"
    echo "  just install-native-system - Install native build system-wide"
    echo "  PREFIX=/usr just install-system - Install to /usr/bin"
    echo "  DESTDIR=/tmp/pkg PREFIX=/usr just install-system - Staged install for packaging"
    echo "  just install-custom /path/to/dir - Install to any custom location"
    echo "  • Position-independent DESTDIR and PREFIX parameters"
    echo "  • DESTDIR + PREFIX pattern for packaging compatibility"
    echo ""
    echo "Performance Optimizations:"
    echo "  • Incremental compilation enabled"
    echo "  • Parallel jobs using all CPU cores"
    echo "  • sccache for build caching (if installed)"
    echo "  • ld.lld or lld for faster linking (if available)"
    echo ""
    echo ""
    echo "🧪 Benchmark Builds:"
    echo "  just benchmark        - Compare build times"

# Format code
fmt:
    #!/usr/bin/env bash
    echo "Formatting code..."
    cargo fmt

# Lint code
lint:
    #!/usr/bin/env bash
    echo "Linting code..."
    cargo clippy

# Run all quality checks
quality: fmt lint test

# Install mfutil system-wide (requires sudo)
# Supports DESTDIR + PREFIX pattern for packaging compatibility
# DESTDIR is prepended to PREFIX for staged installations
# Example: DESTDIR=/tmp/package PREFIX=/usr installs to /tmp/package/usr/bin/
install-system: build
    #!/usr/bin/env bash
    echo "Installing mfutil system-wide..."
    echo "This requires administrator privileges"

    # Get parameter values from environment variables
    # DESTDIR is undefined if not set in environment
    if [ -n "${DESTDIR+x}" ]; then
        DESTDIR_VAL="$DESTDIR"
    else
        DESTDIR_VAL=""
    fi
    PREFIX_VAL="${PREFIX:-/usr/local}"

    # Calculate installation paths using DESTDIR and PREFIX
    if [ -n "$DESTDIR_VAL" ]; then
        # Remove trailing slash from DESTDIR if present
        DESTDIR_CLEAN="${DESTDIR_VAL%/}"
        # Remove leading slash from PREFIX if present
        PREFIX_CLEAN="${PREFIX_VAL#/}"
        # Remove trailing slash from PREFIX if present
        PREFIX_CLEAN="${PREFIX_CLEAN%/}"
        INSTALL_PREFIX="$DESTDIR_CLEAN/$PREFIX_CLEAN"
        echo "DESTDIR: $DESTDIR_VAL"
    else
        # Remove leading slash from PREFIX if present
        PREFIX_CLEAN="${PREFIX_VAL#/}"
        # Remove trailing slash from PREFIX if present
        PREFIX_CLEAN="${PREFIX_CLEAN%/}"
        INSTALL_PREFIX="/$PREFIX_CLEAN"
        echo "DESTDIR: (none)"
    fi
    echo "PREFIX: $PREFIX_VAL"
    echo "Final installation directory: $INSTALL_PREFIX/bin"

    # Detect available privilege escalation tools
    PRIV_ESC=""

    if command -v doas &> /dev/null; then
        # Test if doas is configured by trying a simple command
        if doas true 2>/dev/null; then
            PRIV_ESC="doas"
            echo "Using doas for privilege escalation"
        else
            echo "doas is installed but not configured, skipping..."
        fi
    fi

    if [ -z "$PRIV_ESC" ] && command -v sudo-rs &> /dev/null; then
        # Test if sudo-rs is configured by trying a simple command
        if sudo-rs true 2>/dev/null; then
            PRIV_ESC="sudo-rs"
            echo "Using sudo-rs for privilege escalation"
        else
            echo "sudo-rs is installed but not configured, skipping..."
        fi
    fi

    if [ -z "$PRIV_ESC" ] && command -v sudo &> /dev/null; then
        # Test if sudo is configured by trying a simple command
        if sudo true 2>/dev/null; then
            PRIV_ESC="sudo"
            echo "Using sudo for privilege escalation"
        else
            echo "sudo is installed but not configured, skipping..."
        fi
    fi

    if [ -z "$PRIV_ESC" ] && command -v run0 &> /dev/null; then
        # Test if run0 is configured by trying a simple command
        if run0 true 2>/dev/null; then
            PRIV_ESC="run0"
            echo "Using run0 for privilege escalation (systemd)"
        else
            echo "run0 is installed but not configured, skipping..."
        fi
    fi

    if [ -z "$PRIV_ESC" ]; then
        echo "No configured privilege escalation tool found"
        echo "Available tools that are installed but not configured:"
        command -v doas &> /dev/null && echo "  • doas (installed but not configured)"
        command -v sudo-rs &> /dev/null && echo "  • sudo-rs (installed but not configured)"
        command -v run0 &> /dev/null && echo "  • run0 (installed but not configured)"
        command -v sudo &> /dev/null && echo "  • sudo (installed but not configured)"
        echo ""
        echo "To fix this:"
        echo "  • For doas: Add your user to /etc/doas.conf"
        echo "  • For sudo: Add your user to /etc/sudoers (usually via 'sudo usermod -aG sudo \$USER')"
        echo "  • For run0: Configure polkit rules for your user"
        echo "  • For sudo-rs: Configure /etc/sudo-rs.conf"
        exit 1
    fi

    # Check if we're on Linux
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo "Installing to $INSTALL_PREFIX/bin/..."
        $PRIV_ESC mkdir -p $INSTALL_PREFIX/bin
        $PRIV_ESC cp target/release/mfutil $INSTALL_PREFIX/bin/
        $PRIV_ESC chmod +x $INSTALL_PREFIX/bin/mfutil

        # Install manpages
        echo "Installing manpages to $INSTALL_PREFIX/share/man/man1/..."
        $PRIV_ESC mkdir -p $INSTALL_PREFIX/share/man/man1
        $PRIV_ESC cp man/*.1 $INSTALL_PREFIX/share/man/man1/

        # Add to PATH if not already there (only for non-DESTDIR installs)
        if [ -z "$DESTDIR_VAL" ]; then
            # Use cleaned PREFIX for PATH operations
            if [ -n "$PREFIX_CLEAN" ]; then
                PATH_TO_ADD="$PREFIX_CLEAN/bin"
            else
                PATH_TO_ADD="$PREFIX_VAL/bin"
            fi

            if ! echo "$PATH" | grep -q "$PATH_TO_ADD"; then
                echo "Adding $PREFIX_VAL/bin to your PATH..."
                echo "Add this line to your $HOME/.bashrc or $HOME/.profile:"
                echo "   export PATH=\"$PREFIX_VAL/bin:\$PATH\""
                echo "Or run: export PATH=\"$PREFIX_VAL/bin:\$PATH\""
            fi
        fi

        echo "mfutil installed system-wide!"
        if [ -n "$DESTDIR_VAL" ]; then
            echo "Staged installation ready for packaging"
            echo "DESTDIR: $DESTDIR_VAL"
            if [ -n "$PREFIX_CLEAN" ]; then
                echo "Final location will be: $PREFIX_CLEAN/bin/mfutil"
            else
                echo "Final location will be: $PREFIX_VAL/bin/mfutil"
            fi
        else
            echo "Ready to run: mfutil --help"
        fi
        echo "Location: $INSTALL_PREFIX/bin/mfutil"
    else
        echo "System-wide installation only supported on Linux"
        echo "Try 'just install-local' for user-local installation"
    fi

# Install native build system-wide (requires sudo)
install-native-system: native
    #!/usr/bin/env bash
    echo "Installing native build system-wide..."
    echo "This requires administrator privileges"

    # Get host target
    HOST_TARGET=$(rustc --print host-tuple)
    echo "Installing from target: $HOST_TARGET"

    # Get parameter values from environment variables
    # DESTDIR is undefined if not set in environment
    if [ -n "${DESTDIR+x}" ]; then
        DESTDIR_VAL="$DESTDIR"
    else
        DESTDIR_VAL=""
    fi
    PREFIX_VAL="${PREFIX:-/usr/local}"

    # Calculate installation paths using DESTDIR and PREFIX
    if [ -n "$DESTDIR_VAL" ]; then
        # Remove trailing slash from DESTDIR if present
        DESTDIR_CLEAN="${DESTDIR_VAL%/}"
        # Remove leading slash from PREFIX if present
        PREFIX_CLEAN="${PREFIX_VAL#/}"
        # Remove trailing slash from PREFIX if present
        PREFIX_CLEAN="${PREFIX_CLEAN%/}"
        INSTALL_PREFIX="$DESTDIR_CLEAN/$PREFIX_CLEAN"
        echo "DESTDIR: $DESTDIR_VAL"
    else
        # Remove leading slash from PREFIX if present
        PREFIX_CLEAN="${PREFIX_VAL#/}"
        # Remove trailing slash from PREFIX if present
        PREFIX_CLEAN="${PREFIX_CLEAN%/}"
        INSTALL_PREFIX="/$PREFIX_CLEAN"
        echo "DESTDIR: (none)"
    fi
    echo "PREFIX: $PREFIX_VAL"
    echo "Final installation directory: $INSTALL_PREFIX/bin"

    # Detect available privilege escalation tools
    PRIV_ESC=""

    if command -v doas &> /dev/null; then
        # Test if doas is configured by trying a simple command
        if doas true 2>/dev/null; then
            PRIV_ESC="doas"
            echo "Using doas for privilege escalation"
        else
            echo "doas is installed but not configured, skipping..."
        fi
    fi

    if [ -z "$PRIV_ESC" ] && command -v sudo-rs &> /dev/null; then
        # Test if sudo-rs is configured by trying a simple command
        if sudo-rs true 2>/dev/null; then
            PRIV_ESC="sudo-rs"
            echo "Using sudo-rs for privilege escalation"
        else
            echo "sudo-rs is installed but not configured, skipping..."
        fi
    fi

    if [ -z "$PRIV_ESC" ] && command -v sudo &> /dev/null; then
        # Test if sudo is configured by trying a simple command
        if sudo true 2>/dev/null; then
            PRIV_ESC="sudo"
            echo "Using sudo for privilege escalation"
        else
            echo "sudo is installed but not configured, skipping..."
        fi
    fi

    if [ -z "$PRIV_ESC" ] && command -v run0 &> /dev/null; then
        # Test if run0 is configured by trying a simple command
        if run0 true 2>/dev/null; then
            PRIV_ESC="run0"
            echo "Using run0 for privilege escalation (systemd)"
        else
            echo "run0 is installed but not configured, skipping..."
        fi
    fi

    if [ -z "$PRIV_ESC" ]; then
        echo "No configured privilege escalation tool found"
        echo "Available tools that are installed but not configured:"
        command -v doas &> /dev/null && echo "  • doas (installed but not configured)"
        command -v sudo-rs &> /dev/null && echo "  • sudo-rs (installed but not configured)"
        command -v run0 &> /dev/null && echo "  • run0 (installed but not configured)"
        command -v sudo &> /dev/null && echo "  • sudo (installed but not configured)"
        echo ""
        echo "To fix this:"
        echo "  • For doas: Add your user to /etc/doas.conf"
        echo "  • For sudo: Add your user to /etc/sudoers (usually via 'sudo usermod -aG sudo \$USER')"
        echo "  • For run0: Configure polkit rules for your user"
        echo "  • For sudo-rs: Configure /etc/sudo-rs.conf"
        exit 1
    fi

    # Check if we're on Linux
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo "Installing to $INSTALL_PREFIX/bin/..."
        $PRIV_ESC mkdir -p $INSTALL_PREFIX/bin
        $PRIV_ESC cp target/$HOST_TARGET/release/mfutil $INSTALL_PREFIX/bin/
        $PRIV_ESC chmod +x $INSTALL_PREFIX/bin/mfutil

        # Install manpages
        echo "Installing manpages to $INSTALL_PREFIX/share/man/man1/..."
        $PRIV_ESC mkdir -p $INSTALL_PREFIX/share/man/man1
        $PRIV_ESC cp man/*.1 $INSTALL_PREFIX/share/man/man1/

        # Add to PATH if not already there (only for non-DESTDIR installs)
        if [ -z "$DESTDIR_VAL" ]; then
            # Use cleaned PREFIX for PATH operations
            if [ -n "$PREFIX_CLEAN" ]; then
                PATH_TO_ADD="$PREFIX_CLEAN/bin"
            else
                PATH_TO_ADD="$PREFIX_VAL/bin"
            fi

            if ! echo "$PATH" | grep -q "$PATH_TO_ADD"; then
                echo "Adding $PREFIX_VAL/bin to your PATH..."
                echo "Add this line to your $HOME/.bashrc or $HOME/.profile:"
                echo "   export PATH=\"$PREFIX_VAL/bin:\$PATH\""
                echo "Or run: export PATH=\"$PREFIX_VAL/bin:\$PATH\""
            fi
        fi

        echo "Native build installed system-wide!"
        if [ -n "$DESTDIR_VAL" ]; then
            echo "Staged installation ready for packaging"
            echo "DESTDIR: $DESTDIR_VAL"
            if [ -n "$PREFIX_CLEAN" ]; then
                echo "Final location will be: $PREFIX_CLEAN/bin/mfutil"
            else
                echo "Final location will be: $PREFIX_VAL/bin/mfutil"
            fi
        else
            echo "Ready to run: mfutil --help"
        fi
        echo "Location: $INSTALL_PREFIX/bin/mfutil"
    else
        echo "System-wide installation only supported on Linux"
        echo "Try 'just install-local' for user-local installation"
    fi

# Install mfutil for current user only
install-local: build
    #!/usr/bin/env bash
    echo "Installing mfutil for current user..."

    # Create local bin directory if it doesn't exist
    mkdir -p $HOME/.local/bin

    # Add to PATH if not already there
    if ! echo "$PATH" | grep -q "$HOME/.local/bin"; then
        echo "Adding $HOME/.local/bin to your PATH..."
        echo 'export PATH="$HOME/.local/bin:$PATH"' >> $HOME/.bashrc
        echo 'export PATH="$HOME/.local/bin:$PATH"' >> $HOME/.profile
        echo "PATH updated! Restart your terminal or run: source $HOME/.bashrc"
    fi

    # Copy binary
    cp target/release/mfutil $HOME/.local/bin/
    chmod +x $HOME/.local/bin/mfutil

    # Install manpages
    echo "Installing manpages to $HOME/.local/share/man/man1/..."
    mkdir -p $HOME/.local/share/man/man1
    cp man/*.1 $HOME/.local/share/man/man1/

    echo "mfutil installed locally!"
    echo "Ready to run: mfutil --help"
    echo "Location: $HOME/.local/bin/mfutil"

# Install mfutil to custom directory
install-custom DEST: build
    #!/usr/bin/env bash
    echo "Installing mfutil to {{DEST}}..."

    # Detect available privilege escalation tools
    PRIV_ESC=""

    if command -v doas &> /dev/null; then
        # Test if doas is configured by trying a simple command
        if doas true 2>/dev/null; then
            PRIV_ESC="doas"
            echo "Using doas for privilege escalation"
        else
            echo "doas is installed but not configured, skipping..."
        fi
    fi

    if [ -z "$PRIV_ESC" ] && command -v sudo-rs &> /dev/null; then
        # Test if sudo-rs is configured by trying a simple command
        if sudo-rs true 2>/dev/null; then
            PRIV_ESC="sudo-rs"
            echo "Using sudo-rs for privilege escalation"
        else
            echo "sudo-rs is installed but not configured, skipping..."
        fi
    fi

    if [ -z "$PRIV_ESC" ] && command -v sudo &> /dev/null; then
        # Test if sudo is configured by trying a simple command
        if sudo true 2>/dev/null; then
            PRIV_ESC="sudo"
            echo "Using sudo for privilege escalation"
        else
            echo "sudo is installed but not configured, skipping..."
        fi
    fi

    if [ -z "$PRIV_ESC" ] && command -v run0 &> /dev/null; then
        # Test if run0 is configured by trying a simple command
        if run0 true 2>/dev/null; then
            PRIV_ESC="run0"
            echo "Using run0 for privilege escalation (systemd)"
        else
            echo "run0 is installed but not configured, skipping..."
        fi
    fi

    if [ -z "$PRIV_ESC" ]; then
        echo "No configured privilege escalation tool found"
        echo "Available tools that are installed but not configured:"
        command -v doas &> /dev/null && echo "  • doas (installed but not configured)"
        command -v sudo-rs &> /dev/null && echo "  • sudo-rs (installed but not configured)"
        command -v run0 &> /dev/null && echo "  • run0 (installed but not configured)"
        command -v sudo &> /dev/null && echo "  • sudo (installed but not configured)"
        echo ""
        echo "To fix this:"
        echo "  • For doas: Add your user to /etc/doas.conf"
        echo "  • For sudo: Add your user to /etc/sudoers (usually via 'sudo usermod -aG sudo \$USER')"
        echo "  • For run0: Configure polkit rules for your user"
        echo "  • For sudo-rs: Configure /etc/sudo-rs.conf"
        exit 1
    fi

    # Create destination directory if it doesn't exist
    mkdir -p {{DEST}}

    # Check if we need privilege escalation for this directory
    if [[ "{{DEST}}" == /usr/* ]] || [[ "{{DEST}}" == /opt/* ]] || [[ "{{DEST}}" == /etc/* ]]; then
        echo "Installing to system directory, using privilege escalation..."
        $PRIV_ESC cp target/release/mfutil {{DEST}}/
        $PRIV_ESC chmod +x {{DEST}}/mfutil
    else
        cp target/release/mfutil {{DEST}}/
        chmod +x {{DEST}}/mfutil
    fi

    # Install manpages (assuming {{DEST}} is bin directory)
    MAN_DIR="{{DEST}}/../share/man/man1"
    echo "Installing manpages to $MAN_DIR..."
    if [[ "{{DEST}}" == /usr/* ]] || [[ "{{DEST}}" == /opt/* ]] || [[ "{{DEST}}" == /etc/* ]]; then
        $PRIV_ESC mkdir -p $MAN_DIR
        $PRIV_ESC cp man/*.1 $MAN_DIR/
    else
        mkdir -p $MAN_DIR
        cp man/*.1 $MAN_DIR/
    fi

    echo "mfutil installed to {{DEST}}!"
    echo "Ready to run: {{DEST}}/mfutil --help"

# Show system info
info:
    #!/usr/bin/env bash
    echo "System Information:"
    echo "  OS: $(uname -s) $(uname -r)"
    echo "  CPU cores: $(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 'unknown')"
    echo "  Rust: $(rustc --version)"
    echo "  Cargo: $(cargo --version)"
    echo ""
    echo "Build Tools:"
    if command -v sccache &> /dev/null; then
        echo "  sccache: $(sccache --version | head -1)"
    else
        echo "  sccache: not installed"
    fi
    if command -v lld &> /dev/null; then
        echo "  lld: $(lld --version | head -1)"
    else
        echo "  lld: not installed"
    fi

# Quick development cycle
dev-cycle: check dev
    #!/usr/bin/env bash
    echo "Development cycle complete!"
    echo "Code checks passed"
    echo "Build completed"
    echo "Ready to test: ./target/debug/mfutil"
