# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.kts.

# Keep Wear OS tile and complication service classes
-keep class * extends androidx.wear.tiles.TileService { *; }
-keep class * extends androidx.wear.watchface.complications.datasource.ComplicationDataSourceService { *; }

# Keep Wearable listener service
-keep class * extends com.google.android.gms.wearable.WearableListenerService { *; }

# Keep kotlinx.serialization classes
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.AnnotationsKt
-keepclassmembers class kotlinx.serialization.json.** { *** Companion; }
-keepclasseswithmembers class kotlinx.serialization.json.** { kotlinx.serialization.KSerializer serializer(...); }
-keep,includedescriptorclasses class com.example.subroutine.**$$serializer { *; }
-keepclassmembers class com.example.subroutine.** {
    *** Companion;
}
-keepclasseswithmembers class com.example.subroutine.** {
    kotlinx.serialization.KSerializer serializer(...);
}
