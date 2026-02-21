.DEFAULT_GOAL := apk
.PHONY: jni apk run-on-device

jni:
	cd rust && cargo ndk --target arm64-v8a -o ../java/app/src/main/jniLibs/ build --profile release

apk: jni
	cd java && ./gradlew build

run-on-device: jni
	cd java && ./gradlew assembleDebug --no-daemon && adb install --user 0 -r app/build/outputs/apk/debug/app-debug.apk
	adb shell am start -n local.walkers/.MainActivity
	adb logcat -v color -s walkers RustStdoutStderr
