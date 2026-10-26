package live.akidev.manga

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import live.akidev.manga.app.AkiApp
import live.akidev.manga.app.AppTab
import live.akidev.manga.updates.UpdateAlerts

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val model = (application as AkiApplication).model
        intent?.dataString?.let { model.propose(it) }
        if (intent?.getBooleanExtra(UpdateAlerts.OPEN_UPDATES, false) == true) model.tab = AppTab.Updates
        setContent { AkiApp(model) }
    }

    /** An address opened while the app is running. */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        val model = (application as AkiApplication).model
        intent.dataString?.let { model.propose(it) }
        if (intent.getBooleanExtra(UpdateAlerts.OPEN_UPDATES, false)) model.tab = AppTab.Updates
    }
}
