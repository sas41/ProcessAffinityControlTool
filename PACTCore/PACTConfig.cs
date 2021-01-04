using System;
using System.Text;
using System.Linq;
using System.Collections.Generic;
using System.Diagnostics;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace PACTCore
{
    public class PACTConfig
    {
        // Set of EXE names with their corresponding process configs.
        [JsonInclude]
        public CaseInsensitiveDictionary<ProcessConfig> CustomPerformanceProcesses { get; set; }

        // Set of high performance executable names.
        [JsonInclude]
        public CaseInsensitiveHashSet HighPerformanceProcesses { get; set; }

        // Set of blacklisted executable names.
        [JsonInclude]
        public CaseInsensitiveHashSet Blacklist { get; set; }

        // Applies to all high-performance processes.
        [JsonInclude]
        public ProcessConfig HighPerformanceProcessConfig { get; set; }

        // Applies to all non-high-performance AND non-custom-performance processes.
        [JsonInclude]
        public ProcessConfig DefaultPerformanceProcessConfig { get; set; }

        [JsonInclude]
        public int ScanInterval { get; set; }

        [JsonInclude]
        public int AggressiveScanInterval { get; set; }

        [JsonInclude]
        public bool ForceAggressiveScan { get; set; }



        public PACTConfig()
        {
            CustomPerformanceProcesses = new CaseInsensitiveDictionary<ProcessConfig>();
            HighPerformanceProcesses = new CaseInsensitiveHashSet();
            Blacklist = new CaseInsensitiveHashSet();
            HighPerformanceProcessConfig = new ProcessConfig();
            DefaultPerformanceProcessConfig = new ProcessConfig();

            ScanInterval = 3000;
            AggressiveScanInterval = 20;
            ForceAggressiveScan = false;
        }



        public void RecalculateAffinities()
        {
            DefaultPerformanceProcessConfig.ReCalculateMask();
            HighPerformanceProcessConfig.ReCalculateMask();

            foreach (var kvp in CustomPerformanceProcesses)
            {
                kvp.Value.ReCalculateMask();
            }
        }

        public void AddOToCustomPerformance(string name, ProcessConfig conf)
        {
            if (!string.IsNullOrEmpty(name))
            {
                ClearProcessConfig(name);
                CustomPerformanceProcesses.Add(name, conf);
            }
        }

        public void AddToHighPerformance(string name)
        {
            if (!string.IsNullOrEmpty(name))
            {
                ClearProcessConfig(name);
                HighPerformanceProcesses.Add(name);
            }
        }

        public void AddToBlacklist(string name)
        {
            if (!string.IsNullOrEmpty(name))
            {
                ClearProcessConfig(name);
                Blacklist.Add(name);
            }
        }

        public void ClearProcessConfig(string name)
        {
            if (CustomPerformanceProcesses.ContainsKey(name))
            {
                CustomPerformanceProcesses.Remove(name);
            }

            if (HighPerformanceProcesses.Contains(name))
            {
                HighPerformanceProcesses.Remove(name);
            }

            if (Blacklist.Contains(name))
            {
                Blacklist.Remove(name);
            }
        }
    }
}
