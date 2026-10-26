plugins {
    id("com.android.application")
    kotlin("android")
}

android { namespace = "com.tiffa.lumina.android"; compileSdk = 35
    defaultConfig { applicationId = "com.tiffa.lumina.android"; minSdk = 23; targetSdk = 35; versionCode = 1; versionName = "0.1.0" }
}

kotlin { jvmToolchain(17) }

dependencies {
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.json:json:20240303")
}
