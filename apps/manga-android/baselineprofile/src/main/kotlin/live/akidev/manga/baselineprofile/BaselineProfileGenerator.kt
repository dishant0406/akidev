package live.akidev.manga.baselineprofile

import androidx.benchmark.macro.junit4.BaselineProfileRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.uiautomator.By
import androidx.test.uiautomator.Direction
import androidx.test.uiautomator.Until
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Walks the app the way a person does, so the methods those screens run are compiled ahead of time on install: the start, the tabs,
 * a series, and the reader. Needs a phone with a source installed (the demo server's), or the screens it reaches are the empty ones.
 */
@RunWith(AndroidJUnit4::class)
class BaselineProfileGenerator {
    @get:Rule
    val rule = BaselineProfileRule()

    @Test
    fun generate() = rule.collect(packageName = "live.akidev.manga") {
        pressHome()
        startActivityAndWait()
        device.wait(Until.hasObject(By.text("Discover")), 10_000)
        device.waitForIdle()
        val list = { device.findObject(By.scrollable(true)) }
        list()?.let { it.setGestureMargin(device.displayWidth / 10); it.fling(Direction.DOWN); device.waitForIdle(); it.fling(Direction.UP); device.waitForIdle() }

        for (tab in listOf("Library", "Books", "Updates", "Sources", "Discover")) {
            device.findObject(By.desc(tab))?.click()
            device.waitForIdle()
            Thread.sleep(600)
        }

        device.findObject(By.text("Solo Leveling"))?.click()
        if (device.wait(Until.hasObject(By.text("Chapters")), 5_000)) {
            Thread.sleep(800)
            list()?.let { it.setGestureMargin(device.displayWidth / 10); it.fling(Direction.DOWN); device.waitForIdle(); it.fling(Direction.UP); device.waitForIdle() }
            device.findObject(By.textStartsWith("Chapter"))?.click()
            Thread.sleep(2_500)
            list()?.let { it.setGestureMargin(device.displayWidth / 10); it.fling(Direction.DOWN); device.waitForIdle() }
            device.pressBack()
            device.waitForIdle()
            device.pressBack()
            device.waitForIdle()
        }

        device.findObject(By.desc("Sources"))?.click()
        device.waitForIdle()
        device.findObject(By.desc("Settings"))?.click()
        device.wait(Until.hasObject(By.text("Appearance")), 3_000)
        list()?.let { it.setGestureMargin(device.displayWidth / 10); it.fling(Direction.DOWN); device.waitForIdle() }
    }
}
