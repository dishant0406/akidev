package live.akidev.manga.updates

import android.content.Context
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import live.akidev.manga.AkiApplication
import java.util.concurrent.TimeUnit

/** Android wakes this a few times a day to look for new chapters of saved series, and to finish downloads that were cut short. */
class UpdateWorker(context: Context, parameters: WorkerParameters) : CoroutineWorker(context, parameters) {
    override suspend fun doWork(): Result {
        val model = (applicationContext as AkiApplication).model
        model.refreshLibrary(force = true, background = true)
        model.resumeDownloads()
        return Result.success()
    }

    companion object {
        private const val NAME = "update-library"

        /** Starts looking in the background, or stops when neither alerts nor downloads are wanted. */
        fun schedule(context: Context, wanted: Boolean) {
            val work = WorkManager.getInstance(context)
            if (!wanted) {
                work.cancelUniqueWork(NAME)
                return
            }
            val request = PeriodicWorkRequestBuilder<UpdateWorker>(6, TimeUnit.HOURS)
                .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()).build()
            work.enqueueUniquePeriodicWork(NAME, ExistingPeriodicWorkPolicy.UPDATE, request)
        }
    }
}
