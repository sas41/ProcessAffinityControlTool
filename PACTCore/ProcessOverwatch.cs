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
        private static System.Timers.Timer ScanTimer;


        private PACTConfig config;
        public PACTConfig Config
        {
            get { return config; }
            set { config = value; Config.RecalculateAffinities(); }
        }

        public int AggressiveScanCountdown { get; set; }
        public List<int> ManagedProcesses { get; private set; }
        public List<Process> ProtectedProcesses { get; private set; }

        public ProcessOverwatch()
        {
            Config = new PACTConfig();
            ManagedProcesses = new List<int>();
            ProtectedProcesses = new List<Process>();
            AggressiveScanCountdown = 0;
            Config.RecalculateAffinities();
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

        public void RunScan(bool forced = false)
        {
            if (Config.ForceAggressiveScan || AggressiveScanCountdown == 0 || forced)
            {
                RunAggressiveScan();
                AggressiveScanCountdown = Config.AggressiveScanInterval;
            }
            else
            {
                RunNormalScan();
                AggressiveScanCountdown--;
            }

        }

        // Normal scans only fiddle with processes that are new compared to the last normal scan.
        private void RunNormalScan()
        {
            foreach (var process in Process.GetProcesses())
            {
                int hash = process.GetHashCode();
                try
                {
                    if (!ManagedProcesses.Contains(hash))
                    {
                        SetProcessAffinityAndPriority(process);
                        ManagedProcesses.Add(hash);
                    }
                }
                catch (Exception e)
                {
                    if (!ProtectedProcesses.Contains(process))
                    {
                        ProtectedProcesses.Add(process);
                    }
                }
            }
        }

        // Aggressive Scans apply to both new processes and re-apply to already scanned processes.
        private void RunAggressiveScan()
        {
            Config.RecalculateAffinities();
            ProtectedProcesses.Clear();
            ManagedProcesses.Clear();

            foreach (var process in Process.GetProcesses())
            {
                int hash = process.GetHashCode();
                try
                {
                    SetProcessAffinityAndPriority(process);
                    ManagedProcesses.Add(hash);
                }
                catch (Exception e)
                {
                    ProtectedProcesses.Add(process);
                }
            }
        }

        private void SetProcessAffinityAndPriority(Process process)
        {
            // Todo: Write a case-insensitive comparator extension for dictionaries.
            // This is inhuman...

            IntPtr mask;
            ProcessPriorityClass priority;

            string processName = process.ProcessName;

            if (Config.CheckIfProcessIsBlacklisted(processName))
            {
                return;
            }

            if (Config.CheckIfProcessIsCustomPerformance(processName))
            {
                ProcessConfig conf = Config.GetCustomPerformanceConfig(processName);
                mask = (IntPtr)conf.AffinityMask;
                priority = conf.Priority;
            }
            else if (Config.CheckIfProcessIsHighPerformance(processName))
            {
                mask = (IntPtr)Config.HighPerformanceProcessConfig.AffinityMask;
                priority = Config.HighPerformanceProcessConfig.Priority;
            }
            else
            {
                mask = (IntPtr)Config.DefaultPerformanceProcessConfig.AffinityMask;
                priority = Config.DefaultPerformanceProcessConfig.Priority;
            }

            // Set Process Affinity
            process.ProcessorAffinity = mask;
            process.PriorityClass = priority;
        }
    }
}
