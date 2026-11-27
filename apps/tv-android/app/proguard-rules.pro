# JNA finds the engine's native functions by reflection, so the bindings and JNA itself must keep their names.
-keep class com.sun.jna.** { *; }
-keep class uniffi.** { *; }
-dontwarn java.awt.**
