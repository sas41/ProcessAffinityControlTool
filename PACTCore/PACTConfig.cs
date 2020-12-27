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
        private Dictionary<ulong, ProcessConfig> CustomPerformanceProcesses { get; set; }

        // Set of high performance executable names.
        [JsonProperty]
        private Dictionary<ulong, string> CustomPerformanceHashToNameDictionary { get; set; }

        // Set of high performance executable names.
        [JsonProperty]
        private Dictionary<ulong, string> HighPerformanceProcesses { get; set; }

        // Set of blacklisted executable names.
        [JsonProperty]
        private Dictionary<ulong, string> Blacklist { get; set; }

        // Applies to all high-performance processes.
        [JsonProperty]
        public ProcessConfig HighPerformanceProcessConfig { get; set; }

        // Applies to all non-high-performance AND non-custom-performance processes.
        [JsonProperty]
        public ProcessConfig DefaultPerformanceProcessConfig { get; set; }

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



        public PACTConfig(Dictionary<ulong, ProcessConfig> customPerformanceProcesses = null, Dictionary<ulong, string> customPerformanceHashToNameDictionary = null, Dictionary<ulong, string> highPerformanceProcesses = null, Dictionary<ulong, string> blacklistedProcesses = null, ProcessConfig highPerformanceProcessConfig = null, ProcessConfig defaultPerformanceProcessConfig = null, int scanInterval = 3000, int aggressiveScanInterval = 20, bool forceAggressiveScan = false)
        {
            CustomPerformanceProcesses = (customPerformanceProcesses != null) ? customPerformanceProcesses : new Dictionary<ulong, ProcessConfig>();
            HighPerformanceProcesses = (highPerformanceProcesses != null) ? highPerformanceProcesses : new Dictionary<ulong, string>();
            CustomPerformanceHashToNameDictionary = (customPerformanceHashToNameDictionary != null) ? customPerformanceHashToNameDictionary : new Dictionary<ulong, string>();
            Blacklist = (blacklistedProcesses != null) ? blacklistedProcesses : new Dictionary<ulong, string>();
            HighPerformanceProcessConfig = (highPerformanceProcessConfig != null) ? highPerformanceProcessConfig : new ProcessConfig();
            DefaultPerformanceProcessConfig = (defaultPerformanceProcessConfig != null) ? defaultPerformanceProcessConfig : new ProcessConfig();

            ScanInterval = scanInterval;
            AggressiveScanInterval = aggressiveScanInterval;
            ForceAggressiveScan = forceAggressiveScan;
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


        // Data management methods below...
        public ProcessConfig GetCustomPerformanceConfig(string name)
        {
            string normalized = name.Trim().ToLower();
            ulong key = CustomPerformanceHashToNameDictionary.First(x => x.Value == normalized).Key;

            return CustomPerformanceProcesses[key];
        }

        public IReadOnlyList<string> GetHighPerformanceProcesses()
        {
            return HighPerformanceProcesses.Values.ToList();
        }

        public IReadOnlyList<string> GetCustomPerformanceProcessConfigs()
        {
            return CustomPerformanceHashToNameDictionary.Values.ToList();
        }

        public IReadOnlyList<string> GetBlacklistedProcesses()
        {
            return Blacklist.Values.ToList();
        }




        // The three methods below need to be sped up, massively.
        // Hashtable is a must.
        public bool CheckIfProcessIsHighPerformance(string name)
        {
            ulong hash = PACTHasher.GetUInt64HashCode(name.Trim().ToLower());
            return HighPerformanceProcesses.ContainsKey(hash);
        }

        public bool CheckIfProcessIsCustomPerformance(string name)
        {
            ulong hash = PACTHasher.GetUInt64HashCode(name.Trim().ToLower());
            return CustomPerformanceProcesses.ContainsKey(hash);
        }

        public bool CheckIfProcessIsBlacklisted(string name)
        {
            ulong hash = PACTHasher.GetUInt64HashCode(name.Trim().ToLower());
            return Blacklist.ContainsKey(hash);
        }



        public void AddOrUpdate(string name, ProcessConfig conf = null)
        {
            ulong hash = PACTHasher.GetUInt64HashCode(name.Trim().ToLower());
            ClearProcessConfig(hash);

            if (conf == null)
            {
                this.HighPerformanceProcesses.Add(hash, name.Trim());
            }
            else
            {
                this.CustomPerformanceProcesses.Add(hash, conf);
                this.CustomPerformanceHashToNameDictionary.Add(hash, name.Trim());
            }
        }

        public void AddToBlacklist(string name)
        {
            ulong hash = PACTHasher.GetUInt64HashCode(name.Trim().ToLower());
            ClearProcessConfig(hash);
            this.Blacklist.Add(hash, name.Trim());
        }

        public void ClearProcessConfig(ulong hash)
        {
            if (HighPerformanceProcesses.ContainsKey(hash))
            {
                HighPerformanceProcesses.Remove(hash);
            }

            if (CustomPerformanceProcesses.ContainsKey(hash))
            {
                CustomPerformanceProcesses.Remove(hash);
            }

            if (CustomPerformanceHashToNameDictionary.ContainsKey(hash))
            {
                CustomPerformanceHashToNameDictionary.Remove(hash);
            }

            if (Blacklist.ContainsKey(hash))
            {
                Blacklist.Remove(hash);
            }
        }

        public void ClearProcessConfig(string name)
        {
            ulong hash = PACTHasher.GetUInt64HashCode(name.Trim().ToLower());
            ClearProcessConfig(hash);
        }

        public void ClearHighPerformanceProcessList()
        {
            HighPerformanceProcesses.Clear();
        }

        public void ClearCustomPerformanceProcessList()
        {
            CustomPerformanceProcesses.Clear();
            CustomPerformanceHashToNameDictionary.Clear();
        }

        public void ClearBlacklist()
        {
            Blacklist.Clear();
        }
    }
}
