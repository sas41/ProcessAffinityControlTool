using System;
using System.Text;
using System.Linq;
using System.Collections.Generic;
using System.Diagnostics;
using Newtonsoft.Json;

namespace PACTCore
{
    public class PACTConfig
    {
        // Set of EXE names with their corresponding process configs.
        [JsonProperty]
        public Dictionary<string, ProcessConfig> CustomPriorityProcessConfigs { get; set; }

        // Applies to all high-priority processes.
        [JsonProperty]
        public ProcessConfig HighPriorityProcessConfig { get; set; }

        // Applies to all non-high-priority AND non-custom-priority processes.
        [JsonProperty]
        public ProcessConfig DefaultPriorityProcessConfig { get; set; }

        private int scanInterval;
        [JsonProperty]
        public int ScanInterval
        {
            get
            {
                return scanInterval;
            }
            set
            {
                // Minimum of 3000 miliseconds between normal scans.
                // In testing, a value of 3000 proved safest.
                scanInterval = Math.Max(value, 3000);
            }
        }

        private int aggressiveScanInterval;
        [JsonProperty]
        public int AggressiveScanInterval
        {
            get
            {
                return aggressiveScanInterval;
            }
            set
            {
                // Minimum of 5 normal scans.
                aggressiveScanInterval = Math.Max(value, 5);
            }
        }

        [JsonProperty]
        public bool ForceAggressiveScan { get; set; }

        // This constructor is like so, because of automated deserialization by JSON.NET.
        // It ain't broke, so don't fix it.
        public PACTConfig(Dictionary<string, ProcessConfig> processConfigs = null, ProcessConfig highPriorityProcessConfig = null, ProcessConfig defaultPriorityProcessConfig = null, int scanInterval = 3000, int aggressiveScanInterval = 5, bool forceAggressiveScan = false)
        {
            CustomPriorityProcessConfigs = processConfigs;
            if (processConfigs == null)
            {
                CustomPriorityProcessConfigs = new Dictionary<string, ProcessConfig>();
            }

            HighPriorityProcessConfig = highPriorityProcessConfig;
            if (defaultPriorityProcessConfig == null)
            {
                HighPriorityProcessConfig = new ProcessConfig(Enumerable.Range(0, Environment.ProcessorCount).ToList(), 2);
            }

            DefaultPriorityProcessConfig = defaultPriorityProcessConfig;
            if (defaultPriorityProcessConfig == null)
            {
                DefaultPriorityProcessConfig = new ProcessConfig(Enumerable.Range(0, Environment.ProcessorCount).ToList(), 2);
            }

            ScanInterval = scanInterval;
            AggressiveScanInterval = aggressiveScanInterval;
            ForceAggressiveScan = forceAggressiveScan;
        }

    }
}
