# Android build environment (SDK and JDK under $HOME, shared with timlo).
# Sourced by scripts/phone-install.sh; source it yourself to run
# `npx tauri android …` or gradle directly. Paths can be overridden.
export JAVA_HOME="${FINCO_JAVA_HOME:-$HOME/.local/share/jdk/jdk-21.0.13+9}"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
export NDK_HOME="${NDK_HOME:-$ANDROID_HOME/ndk/27.2.12479018}"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:$PATH"
