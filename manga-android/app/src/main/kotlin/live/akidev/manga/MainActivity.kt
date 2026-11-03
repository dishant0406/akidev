package live.akidev.manga

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import live.akidev.manga.app.AkiApp
import live.akidev.manga.app.AppModel
import live.akidev.manga.app.AppTab
import live.akidev.manga.updates.UpdateAlerts

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val model = (application as AkiApplication).model
        model.glass.watch(window)
        // Only the first start acts on the address or alert that opened the app; starting again after the system killed it must not replay it.
        if (savedInstanceState == null) {
            intent?.let { opened(it, model) }
            if (intent?.getBooleanExtra(UpdateAlerts.OPEN_UPDATES, false) == true) model.tab = AppTab.Updates
        }
        setContent { AkiApp(model) }
    }

    /** An address opened while the app is running. */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        val model = (application as AkiApplication).model
        opened(intent, model)
        if (intent.getBooleanExtra(UpdateAlerts.OPEN_UPDATES, false)) model.tab = AppTab.Updates
    }

    /** What an intent brings: a repository address (`akimanga://`), or a file from another app. */
    private fun opened(intent: Intent, model: AppModel) {
        val sent = if (intent.action == Intent.ACTION_SEND) androidx.core.content.IntentCompat.getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java) else null
        val data = intent.data
        when {
            sent != null -> model.openFile(sent)
            data == null -> {}
            data.scheme == "content" || data.scheme == "file" -> model.openFile(data)
            else -> model.propose(data.toString())
        }
    }
}
