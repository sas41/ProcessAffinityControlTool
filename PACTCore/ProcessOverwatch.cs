using System;
using System.Text;
using System.Linq;
using System.Diagnostics;
using System.Collections.Generic;
using System.Threading;
using System.Timers;
using System.Threading.Tasks;
using System.ComponentModel;

namespace PACTCore
{
    public class ProcessOverwatch
    {

        private BackgroundWorker ScanBackgroundWorker { get; set; }


        private System.Timers.Timer ScanTimer;

        private PACTConfig userConfig;
        public PACTConfig UserConfig
        {
            get { return userConfig; }
            set { userConfig = value; UserConfig.RecalculateAffinities(); }
        }

        public PACTConfig PausedConfig { get; set; }

        public PACTConfig ActiveConfig { get; set; }

        public List<Process> ManagedProcesses { get; private set; }
        public List<Process> ProtectedProcesses { get; private set; }

        public bool AutoMode { get; private set; }

        public CaseInsensitiveHashSet AutoModeDetections { get; private set; }

        public Dictionary<int, string> ChildParentPairs { get; private set; }

        public bool FreshScanRequested { get; set; }
        public bool ScannerActive { get; set; }



        public ProcessOverwatch(PACTConfig userConfig)
        {
            PausedConfig = new PACTConfig();
            UserConfig = userConfig;
            ActiveConfig = UserConfig;

            ManagedProcesses = new List<Process>();
            ProtectedProcesses = new List<Process>();

            AutoMode = true;
            AutoModeDetections = new CaseInsensitiveHashSet();
            ChildParentPairs = new Dictionary<int, string>();

            FreshScanRequested = false;
            ScannerActive = true;



            ScanBackgroundWorker = new BackgroundWorker();
            ScanBackgroundWorker.WorkerReportsProgress = false;
            ScanBackgroundWorker.WorkerSupportsCancellation = false;
            ScanBackgroundWorker.DoWork += TriggerScan;

            ScanTimer = new System.Timers.Timer(UserConfig.ScanInterval);
            ScanTimer.Elapsed += ScanBackGroundWorkerTrigger;
            ScanTimer.AutoReset = true;
            ScanTimer.Enabled = true;
        }



        public bool ToggleProcessOverwatch()
        {
            if (ScannerActive)
            {
                ActiveConfig = PausedConfig;
                ScannerActive = false;
            }
            else
            {
                ActiveConfig = UserConfig;
                ScanTimer.Enabled = true;
                ScannerActive = true;
            }
            return ScannerActive;
        }

        public bool ToggleAutoMode()
        {
            if (AutoMode)
            {
                AutoMode = false;
            }
            else
            {
                AutoMode = true;
            }
            return AutoMode;
        }

        public void RequestFreshScan()
        {
            FreshScanRequested = true;
        }



        private void ScanBackGroundWorkerTrigger(Object source, EventArgs e)
        {
            if (!ScanBackgroundWorker.IsBusy)
            {
                ScanBackgroundWorker.RunWorkerAsync();
            }
        }

        private void TriggerScan(Object source, EventArgs e)
        {
            if (ScanTimer.Enabled)
            {
                if (AutoMode)
                {
                    UpdateChildParentPairs();
                }

                ScanAndManage(ActiveConfig, FreshScanRequested);
                FreshScanRequested = false;

                if (ScannerActive == false)
                {
                    ScanTimer.Enabled = false;
                }
            }
        }

        private void UpdateChildParentPairs()
        {
            Dictionary<int, string> currenChildParentPairs = new Dictionary<int, string>();
            CaseInsensitiveHashSet currenAutoModeDetections = new CaseInsensitiveHashSet();

            foreach (var currentProcess in Process.GetProcesses())
            {
                if (currentProcess.ProcessName.ToLower() == "idle" || currentProcess.ProcessName.ToLower() == "system")
                {
                    continue;
                }

                if (ChildParentPairs.ContainsKey(currentProcess.Id))
                {
                    currenChildParentPairs.Add(currentProcess.Id, ChildParentPairs[currentProcess.Id]);
                }
                else
                {
                    using (var performanceCounter = new PerformanceCounter("Process", "Creating Process ID", currentProcess.ProcessName))
                    {
                        try
                        {
                            int pid = (int)performanceCounter.RawValue;
                            Process parent = Process.GetProcessById(pid);
                            currenChildParentPairs.Add(currentProcess.Id, parent.ProcessName);
                        }
                        catch (ArgumentException)
                        {
                            // No parent
                            currenChildParentPairs.Add(currentProcess.Id, "");
                        }
                        catch (InvalidOperationException)
                        {
                            // Process has exited.
                            continue;
                        }
                    }
                }

                if (!string.IsNullOrEmpty(currenChildParentPairs[currentProcess.Id]))
                {
                    try
                    {
                        var parent = currenChildParentPairs[currentProcess.Id];
                        if (UserConfig.AutoModeLaunchers.Contains(parent))
                        {
                            currenAutoModeDetections.Add(currentProcess.ProcessName);
                        }
                    }
                    catch (Exception)
                    {
                        // Parent has exited.
                    }
                }
            }

            ChildParentPairs = currenChildParentPairs;
            AutoModeDetections = currenAutoModeDetections;
        }

        private void ScanAndManage(PACTConfig config, bool forced = false)
        {
            if (forced)
            {
                ManagedProcesses.Clear();
            }

            ProtectedProcesses.Clear();
            List<Process> currentSet = new List<Process>();

            foreach (var process in Process.GetProcesses())
            {
                if (!ManagedProcesses.Contains(process))
                {
                    try
                    {
                        SetProcessAffinityAndPriority(process, config);
                        currentSet.Add(process);
                    }
                    catch (Exception)
                    {
                        ProtectedProcesses.Add(process);
                    }
                }
            }

            ManagedProcesses = currentSet;
        }

        private void SetProcessAffinityAndPriority(Process process, PACTConfig config)
        {
            IntPtr mask;
            ProcessPriorityClass priority;

            string processName = process.ProcessName;

            if (config.Blacklist.Contains(processName))
            {
                return;
            }

            if (config.CustomPerformanceProcesses.ContainsKey(processName))
            {
                ProcessConfig conf = config.CustomPerformanceProcesses[processName];
                mask = (IntPtr)conf.AffinityMask;
                priority = conf.Priority;
            }
            else if (config.HighPerformanceProcesses.Contains(processName))
            {
                mask = (IntPtr)config.HighPerformanceProcessConfig.AffinityMask;
                priority = config.HighPerformanceProcessConfig.Priority;
            }
            else if (AutoMode && AutoModeDetections.Contains(processName))
            {
                mask = (IntPtr)config.HighPerformanceProcessConfig.AffinityMask;
                priority = config.HighPerformanceProcessConfig.Priority;
            }
            else
            {
                mask = (IntPtr)config.DefaultPerformanceProcessConfig.AffinityMask;
                priority = config.DefaultPerformanceProcessConfig.Priority;
            }

            process.ProcessorAffinity = mask;
            process.PriorityClass = priority;
        }

    }
}
