using System;
using System.Text;
using System.Linq;
using System.Diagnostics;
using System.Collections.Generic;
using System.Threading;
using System.Timers;

namespace PACTCore
{
    public class ProcessOverwatch
    {
        public PACTConfig Config { get; set; }
        public List<string> HighPriorityExecutables { get; set; }
        List<Process> managedProcesses;
        int aggressiveScanCountdown = 0;

        private static System.Timers.Timer ScanTimer;

        public ProcessOverwatch()
        {
            Config = new PACTConfig();
            managedProcesses = new List<Process>();
            HighPriorityExecutables = new List<string>();
        }

        public void SetTimer()
        {
            // Create a timer with a X second interval.
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

        public int RunScan(bool forced = false)
        {
            int errors = 0;

            if (Config.ForceAggressiveScan || aggressiveScanCountdown == 0 || forced)
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

        // Normal scans only fiddle with processes that are new compared to the last normal scan.
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
                        SetProcessAffinityAndPriority(process);
                        managedProcesses.Add(process);
                    }
                }
                catch (Exception e)
                {
                    errorCount++;
                }
            }

            return errorCount;
        }

        // Aggressive Scans apply to both new processes and re-apply to already scanned processes.
        private int RunAggressiveScan()
        {

            int errorCount = 0;
            List<Process> allProcesses = Process.GetProcesses().OrderBy(process => process.ProcessName).ToList();
            
            managedProcesses = new List<Process>();

            foreach (var process in allProcesses)
            {
                try
                {
                    SetProcessAffinityAndPriority(process);
                    managedProcesses.Add(process);
                }
                catch (Exception e)
                {
                    errorCount++;
                }
            }

            return errorCount;
        }

        private void SetProcessAffinityAndPriority(Process process)
        {
            IntPtr mask;
            ProcessPriorityClass priority;
            if (Config.CustomPriorityProcessConfigs.ContainsKey(process.ProcessName.ToLower()))
            {
                mask = (IntPtr)Config.CustomPriorityProcessConfigs[process.ProcessName.ToLower()].AffinityMask;
                priority = Config.CustomPriorityProcessConfigs[process.ProcessName.ToLower()].Priority;
            }
            else if (HighPriorityExecutables.Contains(process.ProcessName.ToLower()))
            {
                mask = (IntPtr)Config.HighPriorityProcessConfig.AffinityMask;
                priority = Config.HighPriorityProcessConfig.Priority;
            }
            else
            {
                mask = (IntPtr)Config.DefaultPriorityProcessConfig.AffinityMask;
                priority = Config.DefaultPriorityProcessConfig.Priority;
            }

            // Set Process Affinity
            process.ProcessorAffinity = mask;
            process.PriorityClass = priority;
        }
    }
}
