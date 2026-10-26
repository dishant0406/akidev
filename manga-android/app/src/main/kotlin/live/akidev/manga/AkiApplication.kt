package live.akidev.manga

import android.app.Application
import live.akidev.manga.app.AppModel

/** Holds the one [AppModel], so the engine opens once however many times the activity is recreated. */
class AkiApplication : Application() {
    lateinit var model: AppModel
        private set

    override fun onCreate() {
        super.onCreate()
        model = AppModel(this)
    }
}
