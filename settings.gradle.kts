pluginManagement {
    repositories {
        maven("https://maven.cefrium.com") {
            content { includeGroup("com.cefrium") }
        }
        google()
        mavenCentral()
        gradlePluginPortal()
    }
    // The com.cefrium plugin version tracks the SDK version (gradle.properties).
    resolutionStrategy {
        eachPlugin {
            if (requested.id.id == "com.cefrium") {
                useVersion(providers.gradleProperty("cefriumVersion").get())
            }
        }
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        maven("https://maven.cefrium.com") {
            content { includeGroup("com.cefrium") }
        }
    }
}

rootProject.name = "sails"
include(":app")
