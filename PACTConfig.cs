using System;
using System.Text;
using System.Linq;
using System.Collections.Generic;
using System.Diagnostics;
using Newtonsoft.Json;

namespace ProcessAffinityControlTool
{
    class PACTConfig
    {
        [JsonProperty]
        public Dictionary<string, ProcessConfig> ProcessConfigs { get; set; }
        [JsonProperty]
        public ProcessConfig GameConfig { get; set; }
        [JsonProperty]
        public ProcessConfig DefaultConfig { get; set; }
        [JsonProperty]
        public int ScanInterval { get; set; }
        [JsonProperty]
        public int AggressiveScanInterval { get; set; }
        [JsonProperty]
        public bool ForceAggressiveScan { get; set; }

        public PACTConfig(Dictionary<string, ProcessConfig> processConfigs = null, ProcessConfig gameconfig = null, ProcessConfig defaultConfig = null, int scanInterval = 3000, int aggressiveScanInterval = 3, bool forceAggressiveScan = false)
        {
            ProcessConfigs = processConfigs;
            if (processConfigs == null)
            {
                ProcessConfigs = new Dictionary<string, ProcessConfig>();
            }

            GameConfig = gameconfig;
            if (defaultConfig == null)
            {
                GameConfig = new ProcessConfig(Enumerable.Range(0, Environment.ProcessorCount).ToList(), 2);
            }

            DefaultConfig = defaultConfig;
            if (defaultConfig == null)
            {
                DefaultConfig = new ProcessConfig(Enumerable.Range(0, Environment.ProcessorCount).ToList(), 2);
            }

            ScanInterval = scanInterval;
            AggressiveScanInterval = aggressiveScanInterval;
            ForceAggressiveScan = forceAggressiveScan;
        }

    }
}
