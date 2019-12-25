using System;
using System.Text;
using System.Linq;
using System.Diagnostics;
using System.Collections.Generic;
using System.Threading;
using System.Timers;

namespace ProcessAffinityControlTool
{
    class ProcessOverwatch
    {
        public PACTConfig Config { get; set; }
        List<Process> managedProcesses;
        int aggressiveScanCountdown = 0;

        private static System.Timers.Timer ScanTimer;

        public ProcessOverwatch()
        {
            Config = new PACTConfig();
            managedProcesses = new List<Process>();
        }

        public void SetTimer()
        {
            // Create a timer with a two second interval.
            ScanTimer = new System.Timers.Timer(Config.ScanInterval);

            // Hook up the Elapsed event for the timer. 
            ScanTimer.Elapsed += TriggerScan;
            ScanTimer.AutoReset = true;
            ScanTimer.Enabled = true;
        }

        public void PauseTimer()
        {
            ScanTimer.Enabled = false;
        }

        private void TriggerScan(Object source, ElapsedEventArgs e)
        {
            RunScan();
        }

        public int RunScan()
        {
            int errors = 0;

            if (Config.ForceAggressiveScan || aggressiveScanCountdown == 0)
            {
                errors = RunAggressiveScan();
                aggressiveScanCountdown = Config.AggressiveScanInterval;
            }
            else
            {
                errors = RunNormalScan();
                aggressiveScanCountdown--;
            }

            return errors;
        }

        private int RunNormalScan()
        {
            int errorCount = 0;
            List<Process> allProcesses = Process.GetProcesses().OrderBy(process => process.ProcessName).ToList();

            foreach (var process in allProcesses)
            {
                try
                {
                    if (!managedProcesses.Contains(process))
                    {
                        HandleProcess(process);
                        managedProcesses.Add(process);
                    }
                }
                catch (Exception e)
                {
                    errorCount++;
                    //Console.WriteLine(e.Message);
                }
            }

            return errorCount;
        }

        private int RunAggressiveScan()
        {

            int errorCount = 0;
            List<Process> allProcesses = Process.GetProcesses().OrderBy(process => process.ProcessName).ToList();
            
            managedProcesses = new List<Process>();

            foreach (var process in allProcesses)
            {
                try
                {
                    HandleProcess(process);
                    managedProcesses.Add(process);
                }
                catch (Exception e)
                {
                    errorCount++;
                    //Console.WriteLine(e.Message);
                }
            }

            return errorCount;
        }

        private void HandleProcess(Process process)
        {
            IntPtr mask;
            ProcessPriorityClass priority;
            if (Config.ProcessConfigs.ContainsKey(process.ProcessName.ToLower()))
            {
                mask = (IntPtr)Config.ProcessConfigs[process.ProcessName.ToLower()].AffinityMask;
                priority = Config.ProcessConfigs[process.ProcessName.ToLower()].Priority;
            }
            else
            {
                mask = (IntPtr)Config.DefaultConfig.AffinityMask;
                priority = Config.DefaultConfig.Priority;
            }

            // Set Process Affinity
            process.ProcessorAffinity = mask;
            process.PriorityClass = priority;
        }
    }
}
