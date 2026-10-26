package live.akidev.manga

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import live.akidev.manga.app.AkiApp

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val model = (application as AkiApplication).model
        intent?.dataString?.let { model.propose(it) }
        setContent { AkiApp(model) }
    }

    /** An address opened while the app is running. */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        intent.dataString?.let { (application as AkiApplication).model.propose(it) }
    }
}
