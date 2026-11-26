package live.akidev.manga.baselineprofile

import androidx.benchmark.macro.BaselineProfileMode
import androidx.benchmark.macro.CompilationMode
import androidx.benchmark.macro.StartupMode
import androidx.benchmark.macro.StartupTimingMetric
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** Cold start time with nothing compiled, with the baseline profile, and with everything compiled: the profile should land near the last. */
@RunWith(AndroidJUnit4::class)
class StartupBenchmark {
    @get:Rule
    val rule = MacrobenchmarkRule()

    /** Generating the baseline profile runs every test of this module; only the benchmark run (`connectedBenchmarkReleaseAndroidTest`) should measure. */
    @Before fun onlyWhenBenchmarking() {
        val rules = InstrumentationRegistry.getArguments().getString("androidx.benchmark.enabledRules")
        assumeTrue(rules == null || "macrobenchmark" in rules)
    }

    @Test fun coldNone() = start(CompilationMode.None())

    @Test fun coldBaselineProfile() = start(CompilationMode.Partial(BaselineProfileMode.Require))

    @Test fun coldFull() = start(CompilationMode.Full())

    private fun start(mode: CompilationMode) = rule.measureRepeated(
        packageName = "live.akidev.manga",
        metrics = listOf(StartupTimingMetric()),
        compilationMode = mode,
        startupMode = StartupMode.COLD,
        iterations = 8,
    ) {
        pressHome()
        startActivityAndWait()
    }
}
