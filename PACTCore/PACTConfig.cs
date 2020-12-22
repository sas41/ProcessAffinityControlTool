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
        public Dictionary<string, ProcessConfig> CustomPriorityProcessList { get; set; }

        // Set of high priority EXE names.
        [JsonProperty]
        public List<string> HighPriorityProcessList { get; set; }

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



        public PACTConfig(Dictionary<string, ProcessConfig> customPriorityProcessList = null, List<string> highPriorityProcessList = null, ProcessConfig highPriorityProcessConfig = null, ProcessConfig defaultPriorityProcessConfig = null, int scanInterval = 3000, int aggressiveScanInterval = 20, bool forceAggressiveScan = false)
        {
            CustomPriorityProcessList = customPriorityProcessList;
            if (customPriorityProcessList == null)
            {
                CustomPriorityProcessList = new Dictionary<string, ProcessConfig>();
            }

            HighPriorityProcessList = highPriorityProcessList;
            if (HighPriorityProcessList == null)
            {
                HighPriorityProcessList = new List<string>();
            }

            HighPriorityProcessConfig = highPriorityProcessConfig;
            if (defaultPriorityProcessConfig == null)
            {
                HighPriorityProcessConfig = new ProcessConfig(Enumerable.Range(0, Environment.ProcessorCount).ToList());
            }

            DefaultPriorityProcessConfig = defaultPriorityProcessConfig;
            if (defaultPriorityProcessConfig == null)
            {
                DefaultPriorityProcessConfig = new ProcessConfig(Enumerable.Range(0, Environment.ProcessorCount).ToList());
            }

            ScanInterval = scanInterval;
            AggressiveScanInterval = aggressiveScanInterval;
            ForceAggressiveScan = forceAggressiveScan;
        }



        public void RecalculateAffinities()
        {
            DefaultPriorityProcessConfig.ReCalculateMask();
            HighPriorityProcessConfig.ReCalculateMask();
            foreach (var kvp in CustomPriorityProcessList)
            {
                kvp.Value.ReCalculateMask();
            }
        }

        public void AddToHighPriority(string name)
        {
            Clear(name);
            this.HighPriorityProcessList.Add(name);
        }

        public void AddToCustomPriority(string name, ProcessConfig conf)
        {
            Clear(name);
            this.CustomPriorityProcessList.Add(name, conf);
        }

        public void Clear(string name)
        {
            // Todo: Again, write a case-insensitive comparator extension for dictionaries.
            // This is inhuman...

            string normalizedName = name.ToLower();
            Dictionary<string, string> normalizedListDictionary = HighPriorityProcessList.ToDictionary(x => x.ToLower(), x => x);
            Dictionary<string, string> normalizedDictionary = CustomPriorityProcessList.ToDictionary(x => x.Key.ToLower(), x => x.Key);

            if (normalizedDictionary.ContainsKey(normalizedName))
            {
                string dictionaryName = normalizedDictionary[normalizedName];
                this.CustomPriorityProcessList.Remove(dictionaryName);
            }
            
            if (normalizedListDictionary.ContainsKey(normalizedName))
            {
                string dictionaryName = normalizedListDictionary[normalizedName];
                this.HighPriorityProcessList.Remove(dictionaryName);
            }
        }
    }
}
